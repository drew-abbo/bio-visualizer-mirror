use crate::renderer::pipelines::common::Pipeline;

/// An effect in the rendering pipeline, consisting of a pipeline and its parameters
pub struct Effect {
    /// We need this to bet able to store any kind of pipeline and its params
    /// A lits of effects are itterated over in the renderer so we want to midigate any manual type handling
    pipeline: Box<dyn Pipeline>,
    params: Box<dyn std::any::Any + Send + Sync>,
}

/// Implements a basic API for creating and accessing effect data
impl Effect {
    pub fn new<P: Pipeline + 'static, T: 'static + Send + Sync>(pipeline: P, params: T) -> Self {
        Self {
            pipeline: Box::new(pipeline),
            params: Box::new(params),
        }
    }

    pub fn pipeline(&self) -> &dyn Pipeline {
        self.pipeline.as_ref()
    }

    pub fn set_params<T: 'static + Send + Sync>(&mut self, params: T) {
        self.params = Box::new(params);
    }

    pub fn get_params<T: 'static>(&self) -> Option<&T> {
        self.params.downcast_ref::<T>()
    }

    pub fn get_params_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.params.downcast_mut::<T>()
    }

    pub(crate) fn params_any(&self) -> &dyn std::any::Any {
        self.params.as_ref()
    }
}
