use super::*;

/// A container widget type that enforces single-child composition rules.
///
/// This type serves as a wrapper for widgets that implement [`SingleChild`]
/// behavior, ensuring proper parent-child relationships in the widget
/// hierarchy. The framework automatically handles conversions via [`From`] for
/// any type that implements both [`SingleChild`] and [`Into<Parent>`]. Prefer
/// using composition APIs rather than constructing this directly.
pub struct XSingleChild<'p>(pub(crate) Box<dyn BoxedParent + 'p>);

/// Represents a parent-child pair in widget composition.
///
/// This structure holds a parent widget and an optional child widget,
/// facilitating the composition process. Used internally during widget tree
/// construction to manage hierarchical relationships between components.
pub struct SinglePair<'c, P> {
  pub(super) parent: P,
  pub(super) child: Option<Widget<'c>>,
}

// ------- SingleChild Implementations -------

impl<P: SingleChild> SingleChild for Option<P> {}

impl<P: Parent> Parent for Option<P> {
  fn with_children<'w>(self, mut children: Vec<Widget<'w>>) -> Widget<'w>
  where
    Self: 'w,
  {
    if let Some(p) = self {
      p.with_children(children)
    } else {
      assert_eq!(children.len(), 1, "Either the parent or the child must exist.");
      children.pop().unwrap()
    }
  }
}

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
      Self: Pipe<Value: Into<XSingleChild<'static>>>,
    {}
  }
}

iter_all_pipe_type_to_impl!(impl_single_child_for_pipe);

// ------ Conversion Implementations -------

// Framework conversion infrastructure for single-child composition

impl<'p, P> From<P> for XSingleChild<'p>
where
  P: SingleChild + Parent + 'p,
{
  #[inline]
  fn from(value: P) -> Self { Self(Box::new(value)) }
}

// Final composition step converting SinglePair to XWidget

impl<'s: 'w, 'w, P> RFrom<SinglePair<'s, P>, OtherWidget<dyn Compose>> for Widget<'w>
where
  P: SingleChild + XParent + 'w,
{
  fn r_from(value: SinglePair<'s, P>) -> Self {
    let SinglePair { parent, child } = value;
    let children = child.map_or_else(Vec::new, |child| vec![child]);
    parent.x_with_children(children)
  }
}

impl<'p> RFrom<XSingleChild<'p>, OtherWidget<dyn Compose>> for Widget<'p> {
  #[inline]
  fn r_from(value: XSingleChild<'p>) -> Self { value.0.boxed_with_children(vec![]) }
}

impl<'c, P> std::ops::Deref for SinglePair<'c, P> {
  type Target = P;
  fn deref(&self) -> &Self::Target { &self.parent }
}

impl<'c, P> std::ops::DerefMut for SinglePair<'c, P> {
  fn deref_mut(&mut self) -> &mut Self::Target { &mut self.parent }
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
