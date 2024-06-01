use anyhow::{anyhow, Result};

use gpui::{
    color::{rgbu, ColorU},
    elements::*,
    Axis, Border, ChildView
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneGroup {
    root: Member
}

impl PaneGroup {
    pub fn new(pane_id: usize) -> Self {
        Self {
            root: Member::Pane(pane_id)
        }
    }

    pub fn split(
        &mut self,

        old_pane_id: usize,
        new_pane_id: usize,

        direction: SplitDirection
    ) -> Result<()> {
        match &mut self.root {
            Member::Pane(pane_id) => {
                if *pane_id == old_pane_id {
                    self.root = Member::new_axis(old_pane_id, new_pane_id, direction);
                    
                    Ok(())
                } else {
                    Err(anyhow!("pane não encontrada"))
                }
            }

            Member::Axis(axis) => axis.split(old_pane_id, new_pane_id, direction)
        }
    }

    pub fn remove(&mut self, pane_id: usize) -> Result<bool> {
        match &mut self.root {
            Member::Pane(_) => Ok(false),

            Member::Axis(axis) => {
                if let Some(last_pane) = axis.remove(pane_id)? {
                    self.root = last_pane;
                }

                Ok(true)
            }
        }
    }

    pub fn render<'a>(&self) -> Box<dyn Element> {
        self.root.render()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Member {
    Axis(PaneAxis),
    Pane(usize)
}

impl Member {
    fn new_axis(old_pane_id: usize, new_pane_id: usize, direction: SplitDirection) -> Self {
        use Axis::*;
        use SplitDirection::*;

        let axis = match direction {
            Up | Down => Vertical,
            Left | Right => Horizontal
        };

        let members = match direction {
            Up | Left => vec![Member::Pane(new_pane_id), Member::Pane(old_pane_id)],
            Down | Right => vec![Member::Pane(old_pane_id), Member::Pane(new_pane_id)]
        };

        Member::Axis(PaneAxis { axis, members })
    }

    pub fn render<'a>(&self) -> Box<dyn Element> {
        match self {
            Member::Pane(view_id) => ChildView::new(*view_id).boxed(),
            Member::Axis(axis) => axis.render()
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PaneAxis {
    axis: Axis,
    members: Vec<Member>
}

impl PaneAxis {
    fn split(
        &mut self,

        old_pane_id: usize,
        new_pane_id: usize,

        direction: SplitDirection
    ) -> Result<()> {
        use SplitDirection::*;

        for (idx, member) in self.members.iter_mut().enumerate() {
            match member {
                Member::Axis(axis) => {
                    if axis.split(old_pane_id, new_pane_id, direction).is_ok() {
                        return Ok(());
                    }
                }

                Member::Pane(pane_id) => {
                    if *pane_id == old_pane_id {
                        if direction.matches_axis(self.axis) {
                            match direction {
                                Up | Left => {
                                    self.members.insert(idx, Member::Pane(new_pane_id));
                                }

                                Down | Right => {
                                    self.members.insert(idx + 1, Member::Pane(new_pane_id));
                                }
                            }
                        } else {
                            *member = Member::new_axis(old_pane_id, new_pane_id, direction);
                        }

                        return Ok(());
                    }
                }
            }
        }

        Err(anyhow!("pane não encontrada"))
    }

    fn remove(&mut self, pane_id_to_remove: usize) -> Result<Option<Member>> {
        let mut found_pane = false;
        let mut remove_member = None;

        for (idx, member) in self.members.iter_mut().enumerate() {
            match member {
                Member::Axis(axis) => {
                    if let Ok(last_pane) = axis.remove(pane_id_to_remove) {
                        if let Some(last_pane) = last_pane {
                            *member = last_pane;
                        }

                        found_pane = true;

                        break;
                    }
                }
                Member::Pane(pane_id) => {
                    if *pane_id == pane_id_to_remove {
                        found_pane = true;
                        remove_member = Some(idx);

                        break;
                    }
                }
            }
        }

        if found_pane {
            if let Some(idx) = remove_member {
                self.members.remove(idx);
            }

            if self.members.len() == 1 {
                Ok(self.members.pop())
            } else {
                Ok(None)
            }
        } else {
            Err(anyhow!("pane não encontrada"))
        }
    }

    fn render<'a>(&self) -> Box<dyn Element> {
        let last_member_ix = self.members.len() - 1;

        Flex::new(self.axis)
            .with_children(self.members.iter().enumerate().map(|(ix, member)| {
                let mut member = member.render();

                if ix < last_member_ix {
                    let mut border = Border::new(border_width(), border_color());
                    
                    match self.axis {
                        Axis::Vertical => border.bottom = true,
                        Axis::Horizontal => border.right = true
                    }

                    member = Container::new(member).with_border(border).boxed();
                }

                Expanded::new(1.0, member).boxed()
            })).boxed()
    }
}

#[derive(Clone, Copy)]
pub enum SplitDirection {
    Up,
    Down,
    Left,
    Right
}

impl SplitDirection {
    fn matches_axis(self, orientation: Axis) -> bool {
        use Axis::*;
        use SplitDirection::*;

        match self {
            Up | Down => match orientation {
                Vertical => true,
                Horizontal => false
            },

            Left | Right => match orientation {
                Vertical => false,
                Horizontal => true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // use super::*;
    // use serde_json::json;
    //
    // #[test]
    // fn test_split_and_remove() -> Result<()> {
    //     let mut group = PaneGroup::new(1);
    //
    //     assert_eq!(
    //         serde_json::to_value(&group)?,
    //
    //         json!({
    //             "type": "pane",
    //             "paneId": 1
    //         })
    //     );
    //
    //     group.split(1, 2, SplitDirection::Right)?;
    //
    //     assert_eq!(
    //         serde_json::to_value(&group)?,
    //
    //         json!({
    //             "type": "axis",
    //             "orientation": "horizontal",
    //
    //             "members": [
    //                 {"type": "pane", "paneId": 1},
    //                 {"type": "pane", "paneId": 2}
    //             ]
    //         })
    //     );
    //
    //     group.split(2, 3, SplitDirection::Up)?;
    //
    //     assert_eq!(
    //         serde_json::to_value(&group)?,
    //
    //         json!({
    //             "type": "axis",
    //             "orientation": "horizontal",
    //
    //             "members": [
    //                 {"type": "pane", "paneId": 1},
    //
    //                 {
    //                     "type": "axis",
    //                     "orientation": "vertical",
    //
    //                     "members": [
    //                         {"type": "pane", "paneId": 3},
    //                         {"type": "pane", "paneId": 2}
    //                     ]
    //                 }
    //             ]
    //         })
    //     );
    //
    //     group.split(1, 4, SplitDirection::Right)?;
    //
    //     assert_eq!(
    //         serde_json::to_value(&group)?,
    //
    //         json!({
    //             "type": "axis",
    //             "orientation": "horizontal",
    //
    //             "members": [
    //                 {"type": "pane", "paneId": 1},
    //                 {"type": "pane", "paneId": 4},
    //
    //                 {
    //                     "type": "axis",
    //                     "orientation": "vertical",
    //
    //                     "members": [
    //                         {"type": "pane", "paneId": 3},
    //                         {"type": "pane", "paneId": 2}
    //                     ]
    //                 }
    //             ]
    //         })
    //     );
    //
    //     group.split(2, 5, SplitDirection::Up)?;
    //
    //     assert_eq!(
    //         serde_json::to_value(&group)?,
    //
    //         json!({
    //             "type": "axis",
    //             "orientation": "horizontal",
    //
    //             "members": [
    //                 {"type": "pane", "paneId": 1},
    //                 {"type": "pane", "paneId": 4},
    //
    //                 {
    //                     "type": "axis",
    //                     "orientation": "vertical",
    //
    //                     "members": [
    //                         {"type": "pane", "paneId": 3},
    //                         {"type": "pane", "paneId": 5},
    //                         {"type": "pane", "paneId": 2}
    //                     ]
    //                 }
    //             ]
    //         })
    //     );
    //
    //     assert_eq!(true, group.remove(5)?);
    //
    //     assert_eq!(
    //         serde_json::to_value(&group)?,
    //
    //         json!({
    //             "type": "axis",
    //             "orientation": "horizontal",
    //
    //             "members": [
    //                 {"type": "pane", "paneId": 1},
    //                 {"type": "pane", "paneId": 4},
    //
    //                 {
    //                     "type": "axis",
    //                     "orientation": "vertical",
    //
    //                     "members": [
    //                         {"type": "pane", "paneId": 3},
    //                         {"type": "pane", "paneId": 2}
    //                     ]
    //                 }
    //             ]
    //         })
    //     );
    //
    //     assert_eq!(true, group.remove(4)?);
    //
    //     assert_eq!(
    //         serde_json::to_value(&group)?,
    //
    //         json!({
    //             "type": "axis",
    //             "orientation": "horizontal",
    //
    //             "members": [
    //                 {"type": "pane", "paneId": 1},
    //
    //                 {
    //                     "type": "axis",
    //                     "orientation": "vertical",
    //
    //                     "members": [
    //                         {"type": "pane", "paneId": 3},
    //                         {"type": "pane", "paneId": 2}
    //                     ]
    //                 }
    //             ]
    //         })
    //     );
    //
    //     assert_eq!(true, group.remove(3)?);
    //
    //     assert_eq!(
    //         serde_json::to_value(&group)?,
    //
    //         json!({
    //             "type": "axis",
    //             "orientation": "horizontal",
    //
    //             "members": [
    //                 {"type": "pane", "paneId": 1},
    //                 {"type": "pane", "paneId": 2}
    //             ]
    //         })
    //     );
    //
    //     assert_eq!(true, group.remove(2)?);
    //
    //     assert_eq!(
    //         serde_json::to_value(&group)?,
    //
    //         json!({
    //             "type": "pane",
    //             "paneId": 1
    //         })
    //     );
    //
    //     assert_eq!(false, group.remove(1)?);
    //
    //     assert_eq!(
    //         serde_json::to_value(&group)?,
    //
    //         json!({
    //             "type": "pane",
    //             "paneId": 1
    //         })
    //     );
    //
    //     Ok(())
    // }
}

#[inline(always)]
fn border_width() -> f32 {
    2.0
}

#[inline(always)]
fn border_color() -> ColorU {
    rgbu(0xdb, 0xdb, 0xdc)
}