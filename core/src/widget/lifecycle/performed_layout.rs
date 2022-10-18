use std::cell::RefCell;

use crate::{
  impl_lifecycle, impl_query_self_only,
  prelude::{data_widget::compose_child_as_data_widget, *},
};

#[derive(Declare)]
pub struct PerformedLayoutListener {
  #[declare(builtin, convert=listener_callback(for<'r> FnMut(LifeCycleCtx<'r>)))]
  pub performed_layout: RefCell<Box<dyn for<'r> FnMut(LifeCycleCtx<'r>)>>,
}
impl_lifecycle!(PerformedLayoutListener, performed_layout);