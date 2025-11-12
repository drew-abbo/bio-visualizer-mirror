use crate::renderer::pipelines::common::Pipeline;

pub struct Effect {
    pipeline: Box<dyn Pipeline>,
    params: Box<dyn std::any::Any + Send + Sync>,
}

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
