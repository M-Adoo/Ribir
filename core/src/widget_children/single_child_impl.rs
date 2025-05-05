use super::*;

/// A container widget type that enforces single-child composition rules.
///
/// This type serves as a wrapper for widgets that implement [`SingleChild`]
/// behavior, ensuring proper parent-child relationships in the widget
/// hierarchy. The framework automatically handles conversions via [`From`] for
/// any type that implements both [`SingleChild`] and [`Into<Parent>`]. Prefer
/// using composition APIs rather than constructing this directly.
pub struct XSingleChild<'w>(pub(crate) Widget<'w>);

/// Represents a parent-child pair in widget composition.
///
/// This structure holds a parent widget and an optional child widget,
/// facilitating the composition process. Used internally during widget tree
/// construction to manage hierarchical relationships between components.
pub struct SinglePair<'c, P> {
  pub(super) parent: P,
  pub(super) child: Option<Widget<'c>>,
}

// -------  Option<SingleChild> Composition Extensions -------

/// Extension trait for optional parent widgets enabling child composition.
///
/// Provides a fluent interface for attaching a child widget to an optional
/// parent, handling three possible cases:
/// 1. Some(parent) + Some(child) → Composed widget
/// 2. Some(parent) + None → Parent widget
/// 3. None + Some(child) → Child widget
///
/// # Panics
/// Panics if both parent and child are `None` (empty composition)
pub trait OptionWithSingleChild {
  fn with_child<'w, 'c: 'w, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'w>
  where
    Self: 'w;
}

impl<P> OptionWithSingleChild for Option<P>
where
  for<'p> P: SingleChild + Into<Parent<'p>>,
{
  fn with_child<'w, 'c: 'w, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'w>
  where
    Self: 'w,
  {
    if let Some(parent) = self {
      parent.with_child(child).into_widget_x()
    } else {
      child
        .into()
        .widget
        .expect("Either the parent or the child must exist.")
    }
  }
}

// ------- SingleChild Implementations -------

impl<'p> SingleChild for XSingleChild<'p> {}

impl<T> SingleChild for T where T: StateReader<Value: SingleChild> {}

impl<T: SingleChild> SingleChild for FatObj<T> {}

impl<F: FnOnce() -> W, W: SingleChild> SingleChild for FnWidget<W, F> {}

/// Macro-generated implementations for pipe types
///
/// Applies [`SingleChild`] to all pipe variants that carry single-child widgets
macro_rules! impl_single_child_for_pipe {
  (<$($generics:ident),*>, $pipe:ty) => {
    impl<$($generics),*> SingleChild for $pipe
    where
      Self: InnerPipe<Value: SingleChild>,
    {}
  }
}

iter_all_pipe_type_to_impl!(impl_single_child_for_pipe);

// ------ XWidget Specializations -------

// Specialized implementations for XWidget working with single-child containers

impl<'p> SingleChild for XWidget<'p, OtherWidget<XSingleChild<'p>>> {}

impl<'c> From<XSingleChild<'c>> for XWidget<'c, OtherWidget<XSingleChild<'c>>> {
  fn from(value: XSingleChild<'c>) -> Self { XWidget::<OtherWidget<_>>::new(value.0) }
}

// ------ Conversion Implementations -------

// Framework conversion infrastructure for single-child composition

impl<'p, P> From<P> for XSingleChild<'p>
where
  P: SingleChild + Into<Parent<'p>>,
{
  #[inline]
  fn from(value: P) -> Self { XSingleChild(value.into().0) }
}

impl<'c> From<XWidget<'c, OtherWidget<XSingleChild<'c>>>> for Parent<'c> {
  fn from(value: XWidget<'c, OtherWidget<XSingleChild<'c>>>) -> Self {
    Parent(value.into_widget_x())
  }
}

// Final composition step converting SinglePair to XWidget

impl<'p: 'w, 'c: 'w, 'w, P> From<SinglePair<'c, P>> for XWidget<'w, OtherWidget<dyn Render>>
where
  P: Into<XSingleChild<'p>> + 'w,
{
  fn from(value: SinglePair<'c, P>) -> Self {
    let SinglePair { parent, child } = value;
    let p = parent.into().0;
    let p = if let Some(child) = child { Widget::new(p, vec![child]) } else { p };
    XWidget::new(p)
  }
}

impl<'p: 'w, 'c: 'w, 'w, P> IntoWidget<'w, 0> for SinglePair<'c, P>
where
  P: Into<XSingleChild<'p>> + 'w,
{
  fn into_widget(self) -> Widget<'w> { XWidget::from(self).into_widget_x() }
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
