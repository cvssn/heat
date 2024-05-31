use super::{
    buffer, Anchor, AnchorRangeExt, Buffer, DisplayPoint, Edit, Point, TextSummary, ToOffset
};

use crate::{
    sum_tree::{self, Cursor, SumTree},
    util::find_insertion_index
};

use anyhow::{anyhow, Result};
use gpui::{AppContext, ModelHandle};

use std::{
    cmp::{self, Ordering},
    iter::Take,
    ops::Range
};

use sum_tree::{Dimension, SeekBias};

pub struct FoldMap {
    buffer: ModelHandle<Buffer>,
    transforms: SumTree<Transform>,
    folds: Vec<Range<Anchor>>
}

impl FoldMap {
    pub fn new(buffer: ModelHandle<Buffer>, app: &AppContext) -> Self {
        let text_summary = buffer.as_ref(app).text_summary();

        Self {
            buffer,
            folds: Vec::new(),

            transforms: SumTree::from_item(Transform {
                summary: TransformSummary {
                    buffer: text_summary.clone(),
                    display: text_summary
                },

                display_text: None
            })
        }
    }

    pub fn buffer_rows(&self, start_row: u32) -> Result<BufferRows> {
        if start_row > self.transforms.summary().display.lines.row {
            return Err(anyhow!("linha de exibição inválida {}", start_row));
        }

        let display_point = Point::new(start_row, 0);
        let mut cursor = self.transforms.cursor();

        cursor.seek(&DisplayPoint(display_point), SeekBias::Left);

        Ok(BufferRows {
            display_point,
            cursor
        })
    }

    pub fn len(&self) -> usize {
        self.transforms.summary().display.chars
    }

    pub fn line_len(&self, row: u32, ctx: &AppContext) -> Result<u32> {
        let line_start = self.to_display_offset(DisplayPoint::new(row, 0), ctx)?.0;
        
        let line_end = if row >= self.max_point().row() {
            self.len()
        } else {
            self.to_display_offset(DisplayPoint::new(row + 1, 0), ctx)?
                .0
                - 1
        };

        Ok((line_end - line_start) as u32)
    }

    pub fn chars_at<'a>(&'a self, point: DisplayPoint, app: &'a AppContext) -> Result<Chars<'a>> {
        let offset = self.to_display_offset(point, app)?;
        let mut cursor = self.transforms.cursor();

        cursor.seek(&offset, SeekBias::Right);

        let buffer = self.buffer.as_ref(app);

        Ok(Chars {
            cursor,
            offset: offset.0,

            buffer,
            buffer_chars: None
        })
    }

    pub fn max_point(&self) -> DisplayPoint {
        DisplayPoint(self.transforms.summary().display.lines)
    }

    pub fn rightmost_point(&self) -> DisplayPoint {
        DisplayPoint(self.transforms.summary().display.rightmost_point)
    }

    pub fn fold<T: ToOffset>(
        &mut self,
        ranges: impl IntoIterator<Item = Range<T>>,
        app: &AppContext
    ) -> Result<()> {
        let mut edits = Vec::new();

        let buffer = self.buffer.as_ref(app);

        for range in ranges.into_iter() {
            let start = range.start.to_offset(buffer)?;
            let end = range.end.to_offset(buffer)?;

            edits.push(Edit {
                old_range: start..end,
                new_range: start..end
            });

            let fold = buffer.anchor_after(start)?..buffer.anchor_before(end)?;
            let ix = find_insertion_index(&self.folds, |probe| probe.cmp(&fold, buffer))?;
            
            self.folds.insert(ix, fold);
        }

        edits.sort_unstable_by(|a, b| {
            a.old_range
                .start
                .cmp(&b.old_range.start)
                .then_with(|| b.old_range.end.cmp(&a.old_range.end))
        });

        self.apply_edits(&edits, app)?;

        Ok(())
    }

    pub fn unfold<T: ToOffset>(
        &mut self,
        ranges: impl IntoIterator<Item = Range<T>>,
        app: &AppContext
    ) -> Result<()> {
        let buffer = self.buffer.as_ref(app);

        let mut edits = Vec::new();

        for range in ranges.into_iter() {
            let start = buffer.anchor_before(range.start.to_offset(buffer)?)?;
            let end = buffer.anchor_after(range.end.to_offset(buffer)?)?;

            // remove as dobras que se cruzam e adicione seus intervalos às edições que são passadas para apply_edits
            self.folds.retain(|fold| {
                if fold.start.cmp(&end, buffer).unwrap() > Ordering::Equal
                    || fold.end.cmp(&start, buffer).unwrap() < Ordering::Equal
                {
                    true
                } else {
                    let offset_range = fold.start.to_offset(buffer).unwrap()..fold.end.to_offset(buffer).unwrap();

                    edits.push(Edit {
                        old_range: offset_range.clone(),
                        new_range: offset_range
                    });

                    false
                }
            });
        }

        self.apply_edits(&edits, app)?;

        Ok(())
    }

    pub fn is_line_folded(&self, display_row: u32) -> bool {
        let mut cursor = self.transforms.cursor::<DisplayPoint, DisplayPoint>();
        
        cursor.seek(&DisplayPoint::new(display_row, 0), SeekBias::Right);
        
        while let Some(transform) = cursor.item() {
            if transform.display_text.is_some() {
                return true;
            }

            if cursor.end().row() == display_row {
                cursor.next()
            } else {
                break;
            }
        }

        false
    }

    pub fn to_display_offset(
        &self,
        point: DisplayPoint,
        app: &AppContext
    ) -> Result<DisplayOffset> {
        let mut cursor = self.transforms.cursor::<DisplayPoint, TransformSummary>();
        
        cursor.seek(&point, SeekBias::Right);

        let overshoot = point.0 - cursor.start().display.lines;
        let mut offset = cursor.start().display.chars;

        if !overshoot.is_zero() {
            let transform = cursor
                .item()
                .ok_or_else(|| anyhow!("ponto de exibição {:?} está fora de alcance", point))?;
            
            assert!(transform.display_text.is_none());
            
            let end_buffer_offset = (cursor.start().buffer.lines + overshoot).to_offset(self.buffer.as_ref(app))?;
            offset += end_buffer_offset - cursor.start().buffer.chars;
        }

        Ok(DisplayOffset(offset))
    }

    pub fn to_buffer_point(&self, display_point: DisplayPoint) -> Point {
        let mut cursor = self.transforms.cursor::<DisplayPoint, TransformSummary>();
        cursor.seek(&display_point, SeekBias::Right);
        
        let overshoot = display_point.0 - cursor.start().display.lines;
        cursor.start().buffer.lines + overshoot
    }

    pub fn to_display_point(&self, point: Point) -> DisplayPoint {
        let mut cursor = self.transforms.cursor::<Point, TransformSummary>();
        
        cursor.seek(&point, SeekBias::Right);
        let overshoot = point - cursor.start().buffer.lines;

        DisplayPoint(cmp::min(
            cursor.start().display.lines + overshoot,
            cursor.end().display.lines
        ))
    }

    pub fn apply_edits(&mut self, edits: &[Edit], app: &AppContext) -> Result<()> {
        let buffer = self.buffer.as_ref(app);
        let mut edits = edits.iter().cloned().peekable();

        let mut new_transforms = SumTree::new();
        let mut cursor = self.transforms.cursor::<usize, usize>();

        cursor.seek(&0, SeekBias::Right);

        while let Some(mut edit) = edits.next() {
            new_transforms.push_tree(cursor.slice(&edit.old_range.start, SeekBias::Left));
            
            edit.new_range.start -= edit.old_range.start - cursor.start();
            edit.old_range.start = *cursor.start();

            cursor.seek(&edit.old_range.end, SeekBias::Right);
            cursor.next();

            let mut delta = edit.delta();

            loop {
                edit.old_range.end = *cursor.start();

                if let Some(next_edit) = edits.peek() {
                    if next_edit.old_range.start > edit.old_range.end {
                        break;
                    }

                    let next_edit = edits.next().unwrap();
                    delta += next_edit.delta();

                    if next_edit.old_range.end > edit.old_range.end {
                        edit.old_range.end = next_edit.old_range.end;
                        
                        cursor.seek(&edit.old_range.end, SeekBias::Right);
                        cursor.next();
                    }
                } else {
                    break;
                }
            }

            edit.new_range.end = ((edit.new_range.start + edit.old_extent()) as isize + delta) as usize;

            let anchor = buffer.anchor_before(edit.new_range.start)?;
            let folds_start = find_insertion_index(&self.folds, |probe| probe.start.cmp(&anchor, buffer))?;
            
            let mut folds = self.folds[folds_start..]
                .iter()
                .map(|fold| {
                    fold.start.to_offset(buffer).unwrap()..fold.end.to_offset(buffer).unwrap()
                })
                .peekable();

            while folds
                .peek()
                .map_or(false, |fold| fold.start < edit.new_range.end)
            {
                let mut fold = folds.next().unwrap();
                let sum = new_transforms.summary();

                assert!(fold.start >= sum.buffer.chars);

                while folds
                    .peek()
                    .map_or(false, |next_fold| next_fold.start <= fold.end)
                {
                    let next_fold = folds.next().unwrap();

                    if next_fold.end > fold.end {
                        fold.end = next_fold.end;
                    }
                }

                if fold.start > sum.buffer.chars {
                    let text_summary = buffer.text_summary_for_range(sum.buffer.chars..fold.start);

                    new_transforms.push(Transform {
                        summary: TransformSummary {
                            display: text_summary.clone(),
                            buffer: text_summary
                        },

                        display_text: None
                    });
                }

                if fold.end > fold.start {
                    new_transforms.push(Transform {
                        summary: TransformSummary {
                            display: TextSummary {
                                chars: 1,

                                bytes: '…'.len_utf8(),
                                lines: Point::new(0, 1),

                                first_line_len: 1,
                                rightmost_point: Point::new(0, 1)
                            },

                            buffer: buffer.text_summary_for_range(fold.start..fold.end)
                        },

                        display_text: Some('…')
                    });
                }
            }

            let sum = new_transforms.summary();

            if sum.buffer.chars < edit.new_range.end {
                let text_summary = buffer.text_summary_for_range(sum.buffer.chars..edit.new_range.end);
                
                new_transforms.push(Transform {
                    summary: TransformSummary {
                        display: text_summary.clone(),
                        buffer: text_summary
                    },

                    display_text: None
                });
            }
        }

        new_transforms.push_tree(cursor.suffix());

        drop(cursor);

        self.transforms = new_transforms;

        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Transform {
    summary: TransformSummary,
    display_text: Option<char>
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TransformSummary {
    display: TextSummary,
    buffer: TextSummary
}

impl sum_tree::Item for Transform {
    type Summary = TransformSummary;

    fn summary(&self) -> Self::Summary {
        self.summary.clone()
    }
}

impl<'a> std::ops::AddAssign<&'a Self> for TransformSummary {
    fn add_assign(&mut self, other: &'a Self) {
        self.buffer += &other.buffer;
        self.display += &other.display;
    }
}

impl<'a> Dimension<'a, TransformSummary> for TransformSummary {
    fn add_summary(&mut self, summary: &'a TransformSummary) {
        *self += summary;
    }
}

pub struct BufferRows<'a> {
    cursor: Cursor<'a, Transform, DisplayPoint, TransformSummary>,
    display_point: Point
}

impl<'a> Iterator for BufferRows<'a> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        while self.display_point > self.cursor.end().display.lines {
            self.cursor.next();
            if self.cursor.item().is_none() {
                // todo: retornar um bool de next?

                break;
            }
        }

        if self.cursor.item().is_some() {
            let overshoot = self.display_point - self.cursor.start().display.lines;
            let buffer_point = self.cursor.start().buffer.lines + overshoot;
            
            self.display_point.row += 1;
            
            Some(buffer_point.row)
        } else {
            None
        }
    }
}

pub struct Chars<'a> {
    cursor: Cursor<'a, Transform, DisplayOffset, TransformSummary>,
    offset: usize,
    buffer: &'a Buffer,
    buffer_chars: Option<Take<buffer::Chars<'a>>>
}

impl<'a> Iterator for Chars<'a> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(c) = self.buffer_chars.as_mut().and_then(|chars| chars.next()) {
            self.offset += 1;

            return Some(c);
        }

        if self.offset == self.cursor.end().display.chars {
            self.cursor.next();
        }

        self.cursor.item().and_then(|transform| {
            if let Some(c) = transform.display_text {
                self.offset += 1;

                Some(c)
            } else {
                let overshoot = self.offset - self.cursor.start().display.chars;
                
                let buffer_start = self.cursor.start().buffer.chars + overshoot;
                let char_count = self.cursor.end().buffer.chars - buffer_start;
                
                self.buffer_chars = Some(self.buffer.chars_at(buffer_start).unwrap().take(char_count));
                
                self.next()
            }
        })
    }
}

impl<'a> Dimension<'a, TransformSummary> for DisplayPoint {
    fn add_summary(&mut self, summary: &'a TransformSummary) {
        self.0 += &summary.display.lines;
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, Ord, PartialOrd, PartialEq)]
pub struct DisplayOffset(usize);

impl<'a> Dimension<'a, TransformSummary> for DisplayOffset {
    fn add_summary(&mut self, summary: &'a TransformSummary) {
        self.0 += &summary.display.chars;
    }
}

impl<'a> Dimension<'a, TransformSummary> for Point {
    fn add_summary(&mut self, summary: &'a TransformSummary) {
        *self += &summary.buffer.lines;
    }
}

impl<'a> Dimension<'a, TransformSummary> for usize {
    fn add_summary(&mut self, summary: &'a TransformSummary) {
        *self += &summary.buffer.chars;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::sample_text;
    use gpui::App;

    #[test]
    fn test_basic_folds() -> Result<()> {
        let mut app = App::new()?;

        let buffer = app.add_model(|_| Buffer::new(0, sample_text(5, 6)));
        let mut map = app.read(|app| FoldMap::new(buffer.clone(), app));

        app.read(|app| {
            map.fold(
                vec![
                    Point::new(0, 2)..Point::new(2, 2),
                    Point::new(2, 4)..Point::new(4, 1)
                ],

                app
            )?;

            assert_eq!(map.text(app), "aa…cc…eeeee");

            Ok::<(), anyhow::Error>(())
        })?;

        let edits = buffer.update(&mut app, |buffer, ctx| {
            let start_version = buffer.version.clone();

            buffer.edit(
                vec![
                    Point::new(0, 0)..Point::new(0, 1),
                    Point::new(2, 3)..Point::new(2, 3)
                ],

                "123",

                Some(ctx)
            )?;

            Ok::<_, anyhow::Error>(buffer.edits_since(start_version).collect::<Vec<_>>())
        })?;

        app.read(|app| {
            map.apply_edits(&edits, app)?;
            
            assert_eq!(map.text(app), "123a…c123c…eeeee");

            Ok::<(), anyhow::Error>(())
        })?;

        let edits = buffer.update(&mut app, |buffer, ctx| {
            let start_version = buffer.version.clone();

            buffer.edit(Some(Point::new(2, 6)..Point::new(4, 3)), "456", Some(ctx))?;
            
            Ok::<_, anyhow::Error>(buffer.edits_since(start_version).collect::<Vec<_>>())
        })?;

        app.read(|app| {
            map.apply_edits(&edits, app)?;
            assert_eq!(map.text(app), "123a…c123456eee");

            map.unfold(Some(Point::new(0, 4)..Point::new(0, 4)), app)?;
            assert_eq!(map.text(app), "123aaaaa\nbbbbbb\nccc123456eee");

            Ok(())
        })
    }

    #[test]
    fn test_overlapping_folds() -> Result<()> {
        let mut app = App::new()?;
        let buffer = app.add_model(|_| Buffer::new(0, sample_text(5, 6)));

        app.read(|app| {
            let mut map = FoldMap::new(buffer.clone(), app);

            map.fold(
                vec![
                    Point::new(0, 2)..Point::new(2, 2),
                    Point::new(0, 4)..Point::new(1, 0),
                    Point::new(1, 2)..Point::new(3, 2),
                    Point::new(3, 1)..Point::new(4, 1)
                ],

                app
            )?;

            assert_eq!(map.text(app), "aa…eeeee");

            Ok(())
        })
    }

    #[test]
    fn test_merging_folds_via_edit() -> Result<()> {
        let mut app = App::new()?;

        let buffer = app.add_model(|_| Buffer::new(0, sample_text(5, 6)));
        let mut map = app.read(|app| FoldMap::new(buffer.clone(), app));

        app.read(|app| {
            map.fold(
                vec![
                    Point::new(0, 2)..Point::new(2, 2),
                    Point::new(3, 1)..Point::new(4, 1)
                ],

                app
            )?;

            assert_eq!(map.text(app), "aa…cccc\nd…eeeee");

            Ok::<(), anyhow::Error>(())
        })?;

        let edits = buffer.update(&mut app, |buffer, ctx| {
            let start_version = buffer.version.clone();

            buffer.edit(Some(Point::new(2, 2)..Point::new(3, 1)), "", Some(ctx))?;
            
            Ok::<_, anyhow::Error>(buffer.edits_since(start_version).collect::<Vec<_>>())
        })?;

        app.read(|app| {
            map.apply_edits(&edits, app)?;

            assert_eq!(map.text(app), "aa…eeeee");

            Ok(())
        })
    }

    #[test]
    fn test_random_folds() -> Result<()> {
        use crate::editor::ToPoint;
        use crate::util::RandomCharIter;

        use rand::prelude::*;

        for seed in 0..100 {
            println!("{:?}", seed);

            let mut rng = StdRng::seed_from_u64(seed);

            let mut app = App::new()?;

            let buffer = app.add_model(|_| {
                let len = rng.gen_range(0..10);

                let text = RandomCharIter::new(&mut rng).take(len).collect::<String>();
                
                Buffer::new(0, text)
            });

            let mut map = app.read(|app| FoldMap::new(buffer.clone(), app));

            app.read(|app| {
                let buffer = buffer.as_ref(app);

                let fold_count = rng.gen_range(0..10);
                let mut fold_ranges: Vec<Range<usize>> = Vec::new();
                
                for _ in 0..fold_count {
                    let end = rng.gen_range(0..buffer.len() + 1);
                    let start = rng.gen_range(0..end + 1);

                    fold_ranges.push(start..end);
                }

                map.fold(fold_ranges, app)?;

                let mut expected_text = buffer.text();

                for fold_range in map.merged_fold_ranges(app).into_iter().rev() {
                    expected_text.replace_range(fold_range.start..fold_range.end, "…");
                }

                assert_eq!(map.text(app), expected_text);

                for fold_range in map.merged_fold_ranges(app) {
                    let display_point = map.to_display_point(fold_range.start.to_point(buffer).unwrap());
                    
                    assert!(map.is_line_folded(display_point.row()));
                }

                Ok::<(), anyhow::Error>(())
            })?;

            let edits = buffer.update(&mut app, |buffer, ctx| {
                let start_version = buffer.version.clone();
                let edit_count = rng.gen_range(1..10);

                buffer.randomly_edit(&mut rng, edit_count, Some(ctx));
                
                Ok::<_, anyhow::Error>(buffer.edits_since(start_version).collect::<Vec<_>>())
            })?;

            app.read(|app| {
                map.apply_edits(&edits, app)?;

                let buffer = map.buffer.as_ref(app);
                let mut expected_text = buffer.text();

                for fold_range in map.merged_fold_ranges(app).into_iter().rev() {
                    expected_text.replace_range(fold_range.start..fold_range.end, "…");
                }

                assert_eq!(map.text(app), expected_text);

                Ok::<(), anyhow::Error>(())
            })?;
        }

        Ok(())
    }

    #[test]
    fn test_buffer_rows() -> Result<()> {
        let mut app = App::new()?;
        let text = sample_text(6, 6) + "\n";
        let buffer = app.add_model(|_| Buffer::new(0, text));

        app.read(|app| {
            let mut map = FoldMap::new(buffer.clone(), app);

            map.fold(
                vec![
                    Point::new(0, 2)..Point::new(2, 2),
                    Point::new(3, 1)..Point::new(4, 1)
                ],

                app
            )?;

            assert_eq!(map.text(app), "aa…cccc\nd…eeeee\nffffff\n");

            assert_eq!(map.buffer_rows(0)?.collect::<Vec<_>>(), vec![0, 3, 5, 6]);
            assert_eq!(map.buffer_rows(3)?.collect::<Vec<_>>(), vec![6]);

            Ok(())
        })
    }

    impl FoldMap {
        fn text(&self, app: &AppContext) -> String {
            self.chars_at(DisplayPoint(Point::zero()), app)
                .unwrap()
                .collect()
        }

        fn merged_fold_ranges(&self, app: &AppContext) -> Vec<Range<usize>> {
            let buffer = self.buffer.as_ref(app);
            
            let mut fold_ranges = self
                .folds
                .iter()
                .map(|fold| {
                    fold.start.to_offset(buffer).unwrap()..fold.end.to_offset(buffer).unwrap()
                })
                .peekable();

            let mut merged_ranges = Vec::new();

            while let Some(mut fold_range) = fold_ranges.next() {
                while let Some(next_range) = fold_ranges.peek() {
                    if fold_range.end >= next_range.start {
                        if next_range.end > fold_range.end {
                            fold_range.end = next_range.end;
                        }

                        fold_ranges.next();
                    } else {
                        break;
                    }
                }

                if fold_range.end > fold_range.start {
                    merged_ranges.push(fold_range);
                }
            }

            merged_ranges
        }
    }
}