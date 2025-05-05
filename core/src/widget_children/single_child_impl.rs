use super::*;

/// A container type for widgets implementing [`SingleChild`] composition.
///
/// The framework automatically provides [`From`] conversions for valid widget
/// types (those implementing both [`SingleChild`] and [`Into<Parent>`]). You
/// should never need to construct this directly - use the framework's
/// composition APIs instead.
pub struct XSingleChild<'w>(pub(crate) Widget<'w>);

// -----------  SingleChild implementations ------------

impl<'p> SingleChild for XSingleChild<'p> {
  fn with_child<'c, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'c>
  where
    Self: 'c,
  {
    compose_single_child(self, child.into())
  }
}

impl<T> SingleChild for T
where
  T: StateReader<Value: SingleChild> + IntoWidgetX<'static, OtherWidget<dyn Render>>,
{
  fn with_child<'c, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'c> {
    compose_single_child(self.into(), child.into())
  }
}

impl<'p> SingleChild for XWidget<'p, OtherWidget<XSingleChild<'p>>> {
  fn with_child<'c, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'c>
  where
    Self: 'c,
  {
    compose_single_child(self.into(), child.into())
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

pub fn compose_single_child<'c, K>(
  parent: XSingleChild<'c>, child: OptionWidget<'c, K>,
) -> Widget<'c> {
  if let Some(child) = child.widget { Widget::new(parent.0, vec![child]) } else { parent.0 }
}

// ----- General Conversion `XSingleChild` implementations ------

impl<'p, P> From<P> for XSingleChild<'p>
where
  P: SingleChild + Into<Parent<'p>>,
{
  #[inline]
  fn from(value: P) -> Self { XSingleChild(value.into().0) }
}

impl<'c> From<XSingleChild<'c>> for XWidget<'c, OtherWidget<XSingleChild<'c>>> {
  fn from(value: XSingleChild<'c>) -> Self { XWidget::<OtherWidget<_>>::new(value.0) }
}

impl<'c> From<XWidget<'c, OtherWidget<XSingleChild<'c>>>> for Parent<'c> {
  fn from(value: XWidget<'c, OtherWidget<XSingleChild<'c>>>) -> Self {
    Parent(value.into_widget_x())
  }
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
