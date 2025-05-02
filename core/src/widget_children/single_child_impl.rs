use super::*;
use crate::pipe::{InnerPipe, OptionPipeWidget};

impl<'c, W, K> From<W> for OptionWidget<'c, K>
where
  W: Into<XWidget<'c, K>>,
{
  fn from(value: W) -> Self {
    OptionWidget { widget: Some(value.into().into_widget_x()), _kind: PhantomData }
  }
}

impl<'c, W, K> From<Option<W>> for OptionWidget<'c, ConvertFrom<K>>
where
  W: Into<XWidget<'c, ConvertFrom<K>>>,
{
  fn from(value: Option<W>) -> Self {
    let w = value.map(Into::into).map(|w| w.widget);
    OptionWidget { widget: w, _kind: PhantomData }
  }
}

/// This trait allows an `Option` of `SingleChild` to compose child.
pub trait OptionSingleChild {
  fn with_child<'c, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'c>;
}

impl<P> OptionSingleChild for Option<P>
where
  P: SingleChild,
{
  fn with_child<'c, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'c> {
    if let Some(parent) = self {
      parent.with_child(child)
    } else {
      child
        .into()
        .widget
        .expect("Either the parent or the child must exist.")
    }
  }
}

impl<T: SingleChild> SingleChild for FatObj<T> {
  fn with_child<'c, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'c> {
    self
      .map(|parent| parent.with_child(child))
      .into_widget()
  }

  fn into_parent(self: Box<Self>) -> Widget<'static> {
    let this = *self;
    if !this.has_class() {
      this.into_widget()
    } else {
      panic!("A FatObj should not have a class attribute when acting as a single parent")
    }
  }
}

macro_rules! impl_single_child_methods_for_pipe {
  () => {
    fn with_child<'c, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'c> {
      compose_single_child(self.into_parent_widget(), child.into())
    }

    fn into_parent(self: Box<Self>) -> Widget<'static> { self.into_parent_widget() }
  };
}

impl<V> SingleChild for Box<dyn Pipe<Value = V>>
where
  V: OptionPipeWidget<RENDER> + 'static,
  <V as OptionPipeWidget<RENDER>>::Widget: SingleChild,
{
  impl_single_child_methods_for_pipe!();
}

macro_rules! impl_single_child_methods_for_pipe_option {
  () => {
    fn with_child<'c, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'c> {
      let parent = self.into_parent_widget();
      compose_single_child(parent, child.into())
    }

    fn into_parent(self: Box<Self>) -> Widget<'static> { self.into_parent_widget() }
  };
}
impl<S, V, F> SingleChild for MapPipe<V, S, F>
where
  Self: InnerPipe<Value = V>,
  V: OptionPipeWidget<RENDER>,
  <V as OptionPipeWidget<RENDER>>::Widget: SingleChild,
{
  impl_single_child_methods_for_pipe_option!();
}

impl<S, V, F> SingleChild for FinalChain<V, S, F>
where
  Self: InnerPipe<Value = V>,
  V: OptionPipeWidget<RENDER>,
  <V as OptionPipeWidget<RENDER>>::Widget: SingleChild,
{
  impl_single_child_methods_for_pipe_option!();
}

impl<T> SingleChild for T
where
  T: StateReader<Value: SingleChild> + IntoWidget<'static, RENDER>,
{
  fn with_child<'c, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'c> {
    compose_single_child(self.into_widget(), child.into())
  }

  #[inline]
  fn into_parent(self: Box<Self>) -> Widget<'static> { self.into_widget() }
}

impl SingleChild for Box<dyn SingleChild> {
  fn with_child<'c, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'c> {
    compose_single_child(self.into_parent(), child.into())
  }

  fn into_parent(self: Box<Self>) -> Widget<'static> { (*self).into_parent() }
}

pub fn compose_single_child<'c, K>(parent: Widget<'c>, child: OptionWidget<'c, K>) -> Widget<'c> {
  if let Some(child) = child.widget { Widget::new(parent, vec![child]) } else { parent }
}
#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_helper::MockBox;

  #[test]
  fn pair_with_child() {
    let mock_box = MockBox { size: ZERO_SIZE };
    let _ = |_: &BuildCtx| -> Widget {
      mock_box
        .clone()
        .with_child(mock_box.clone().with_child(mock_box))
        .into_widget()
    };
  }

  #[test]
  fn fix_mock_box_compose_pipe_option_widget() {
    fn _x(w: BoxPipe<Option<BoxFnWidget<'static>>>) {
      MockBox { size: ZERO_SIZE }.with_child(w.into_pipe());
    }
  }
}
