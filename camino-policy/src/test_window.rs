//! `TestWindow`: a `PolicyWindow` whose facts tests set, and which records every request.

use smithay::utils::{FrameExtents, Logical, Point, Rectangle, Size};

use crate::{OutputId, PolicyWindow, SizeHints, WindowId, WindowStates, WindowType};

/// A request policy made on a `TestWindow`.
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    SetGeometry(Rectangle<i32, Logical>, WindowStates),
    SetActivated(bool),
    Close,
    SetOutputs(Vec<OutputId>),
}

#[derive(Debug, Clone)]
pub struct TestWindow {
    pub id: WindowId,
    pub window_type: WindowType,
    pub parent: Option<WindowId>,
    pub size: Size<i32, Logical>,
    pub hints: SizeHints,
    pub requested_position: Option<Point<i32, Logical>>,
    pub input: bool,
    pub ssd: bool,
    pub extents: FrameExtents<i32, Logical>,
    /// The frame's extents while maximised.
    pub maximized_extents: FrameExtents<i32, Logical>,
    pub calls: Vec<Call>,
}

impl TestWindow {
    /// A normal CSD window of the given size.
    pub fn new(id: u64, w: i32, h: i32) -> Self {
        Self {
            id: WindowId(id),
            window_type: WindowType::Normal,
            parent: None,
            size: Size::new(w, h),
            hints: SizeHints::default(),
            requested_position: None,
            input: true,
            ssd: false,
            extents: FrameExtents::default(),
            maximized_extents: FrameExtents::default(),
            calls: Vec::new(),
        }
    }

    pub fn with_type(mut self, window_type: WindowType) -> Self {
        self.window_type = window_type;
        self
    }

    pub fn with_parent(mut self, parent: u64) -> Self {
        self.parent = Some(WindowId(parent));
        self
    }

    /// Makes the window SSD with a frame of the given extents, maximised or not.
    pub fn with_frame(mut self, left: i32, right: i32, top: i32, bottom: i32) -> Self {
        self.ssd = true;
        self.extents = FrameExtents::new(left, right, top, bottom);
        self.maximized_extents = self.extents;
        self
    }

    pub fn with_hints(mut self, min: (i32, i32), max: (i32, i32)) -> Self {
        self.hints = SizeHints {
            min: min.into(),
            max: max.into(),
        };
        self
    }

    pub fn with_requested_position(mut self, x: i32, y: i32) -> Self {
        self.requested_position = Some(Point::new(x, y));
        self
    }

    /// Makes the window take no keyboard input.
    pub fn without_input(mut self) -> Self {
        self.input = false;
        self
    }

    /// The last geometry policy set, if any.
    pub fn last_geometry(&self) -> Option<(Rectangle<i32, Logical>, WindowStates)> {
        self.calls.iter().rev().find_map(|call| match call {
            Call::SetGeometry(geometry, states) => Some((*geometry, *states)),
            _ => None,
        })
    }

    pub fn last_outputs(&self) -> Option<&[OutputId]> {
        self.calls.iter().rev().find_map(|call| match call {
            Call::SetOutputs(outputs) => Some(outputs.as_slice()),
            _ => None,
        })
    }

    pub fn is_activated(&self) -> bool {
        self.calls
            .iter()
            .rev()
            .find_map(|call| match call {
                Call::SetActivated(activated) => Some(*activated),
                _ => None,
            })
            .unwrap_or(false)
    }
}

impl PolicyWindow for TestWindow {
    fn id(&self) -> WindowId {
        self.id
    }

    fn window_type(&self) -> WindowType {
        self.window_type
    }

    fn transient_parent(&self) -> Option<WindowId> {
        self.parent
    }

    fn size(&self) -> Size<i32, Logical> {
        self.size
    }

    fn size_hints(&self) -> SizeHints {
        self.hints
    }

    fn requested_position(&self) -> Option<Point<i32, Logical>> {
        self.requested_position
    }

    fn takes_input(&self) -> bool {
        self.input
    }

    fn pid(&self) -> Option<u32> {
        None
    }

    fn app_id(&self) -> Option<String> {
        None
    }

    fn is_ssd(&self) -> bool {
        self.ssd
    }

    fn frame_extents(&self, maximized: bool) -> FrameExtents<i32, Logical> {
        if maximized {
            self.maximized_extents
        } else {
            self.extents
        }
    }

    fn set_geometry(&mut self, geometry: Rectangle<i32, Logical>, states: WindowStates) {
        self.calls.push(Call::SetGeometry(geometry, states));
    }

    fn set_activated(&mut self, activated: bool) {
        self.calls.push(Call::SetActivated(activated));
    }

    fn close(&mut self) {
        self.calls.push(Call::Close);
    }

    fn set_outputs(&mut self, outputs: &[OutputId]) {
        self.calls.push(Call::SetOutputs(outputs.to_vec()));
    }
}
