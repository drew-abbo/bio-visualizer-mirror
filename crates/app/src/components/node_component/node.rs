use eframe::wgpu::TextureView;
use crate::view::View;

/// A node in the node graph that can render output and display a UI
pub trait Node: View {
    /// Get the unique identifier for this node
    fn id(&self) -> usize;

    /// Get the display label for this node
    fn label(&self) -> &str;

    /// Set the output texture frame for this node to display
    fn set_frame(&mut self, view: TextureView);

    /// Toggle whether to show the frame output (true = show, false = hide)
    fn set_show_frame(&mut self, show: bool);

    /// Get whether the frame output is currently visible
    fn show_frame(&self) -> bool;

    /// Clear the current frame
    fn clear_frame(&mut self);
}