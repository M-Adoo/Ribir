use super::*;
use crate::pipe::*;

// -----------  SingleChild implementations ------------
impl<T> SingleChild for T
where
  T: StateReader<Value: SingleChild> + IntoWidgetX<'static, OtherWidget<dyn Render>>,
{
  fn with_child<'c, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'c> {
    compose_single_child(self.into_widget_x(), child.into())
  }
}

impl<F, W> SingleChild for FnWidget<W, F>
where
  F: FnOnce() -> W,
  W: SingleChild,
{
  fn with_child<'c, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'c>
  where
    Self: 'c,
  {
    let child = child.into().widget;
    let f = FnWidget::new(move || self.call().with_child(child));
    f.into_widget_x()
  }
}

impl<P> SingleChild for Option<P>
where
  P: SingleChild,
{
  fn with_child<'c, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'c>
  where
    Self: 'c,
  {
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
  fn with_child<'c, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'c>
  where
    Self: 'c,
  {
    self
      .map(|parent| parent.with_child(child))
      .into_widget_x()
  }
}

macro_rules! impl_single_child_for_pipe {
  (<$($generics:ident),*>, $pipe:ty) => {
    impl<$($generics),*> SingleChild for $pipe
    where
      Self: InnerPipe<Value = V>,
      V: SingleChild
    {
      fn with_child<'c, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'c> {
        todo!("mack parent into widget")
        // compose_single_child(self.into_parent_widget(), child.into())
      }
    }
  }
}

iter_all_pipe_type_to_impl!(impl_single_child_for_pipe);

pub fn compose_single_child<'c, K>(parent: Widget<'c>, child: OptionWidget<'c, K>) -> Widget<'c> {
  if let Some(child) = child.widget { Widget::new(parent, vec![child]) } else { parent }
}

// ----- General Conversion `XSingleChild` implementations ------

impl<'c, W> From<W> for XSingleChild<'c>
where
  W: SingleChild + IntoWidgetX<'c, OtherWidget<dyn Render>>,
{
  fn from(value: W) -> Self { XSingleChild(value.into_widget_x()) }
}

impl<'c, P> From<FatObj<P>> for XSingleChild<'c>
where
  P: Into<XSingleChild<'c>>,
{
  fn from(value: FatObj<P>) -> Self {
    if !value.has_class() {
      let w = value.map(|p| p.into().0).compose();
      XSingleChild(w)
    } else {
      panic!("A FatObj should not have a class attribute when acting as a single parent")
    }
  }
}

macro_rules! impl_pipe_to_x_single_child {
  (<$($generics:ident),*> , $pipe:ty) => {
    impl<$($generics),*> From<$pipe> for XSingleChild<'static>
    where
      $pipe: InnerPipe,
      V: SingleChild,
    {
      fn from(value: $pipe) -> Self { todo!("wait pipe switch to XWidget impl") }
    }
  };
}
iter_all_pipe_type_to_impl!(impl_pipe_to_x_single_child);

impl<'w, F, W> From<FnWidget<W, F>> for XSingleChild<'w>
where
  F: FnOnce() -> W + 'w,
  W: Into<XSingleChild<'w>> + 'w,
{
  fn from(value: FnWidget<W, F>) -> Self {
    let f = FnWidget::new(move || value.call().into().0);
    XSingleChild(f.into_widget_x())
  }
}

impl<'c> From<XWidget<'c, OtherWidget<XSingleChild<'c>>>> for XSingleChild<'c> {
  fn from(value: XWidget<'c, OtherWidget<XSingleChild<'c>>>) -> Self {
    XSingleChild(value.into_widget_x())
  }
}

/// `SingleChildKind` make we know a `XWidget` is convert from a `SingleChild`
impl<'c> From<XSingleChild<'c>> for XWidget<'c, OtherWidget<XSingleChild<'c>>> {
  fn from(value: XSingleChild<'c>) -> Self { XWidget::<OtherWidget<_>>::new(value.0) }
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
