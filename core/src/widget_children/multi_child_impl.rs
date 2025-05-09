use super::*;

/// A container widget type that enables composition of multiple child widgets.
///
/// This type wraps widgets that implement both [`MultiChild`] and
/// [`Into<Parent>`] traits, providing automatic conversion via the [`From`]
/// trait. It serves as the foundation for multi-child widget hierarchies in the
/// framework.
///
/// # Usage
/// - Never construct directly - use composition APIs like `with_child` instead
/// - Automatic conversions handle wrapping of valid widget types
pub struct XMultiChild<'p>(pub(crate) Box<dyn BoxedParent + 'p>);

/// A paired parent widget with its collected child widgets.
///
/// This structure is used during widget composition to gradually build up
/// a parent-child relationship while maintaining type safety.
///
/// # Type Parameters
/// - `P`: The parent widget type implementing multi-child capabilities
pub struct MultiPair<'a, P> {
  pub(super) parent: P,
  pub(super) children: Vec<Widget<'a>>,
}

impl<'p, P> MultiPair<'p, P> {
  /// Chains additional children to an existing parent-children pair
  ///
  /// # Note
  /// Maintains ownership of the parent widget while extending child collection
  pub fn with_child<'c: 'w, 'w, K: ?Sized>(
    self, child: impl IntoWidgetIter<'c, K>,
  ) -> MultiPair<'w, P>
  where
    'p: 'w,
  {
    let MultiPair { parent, mut children } = self;
    for c in child.into_widget_iter() {
      children.push(c);
    }
    MultiPair { parent, children }
  }
}

// ------ Core Type Conversions ------

/// Enables conversion of any valid MultiChild widget to XMultiChild container
impl<'p, P> From<P> for XMultiChild<'p>
where
  P: Parent + MultiChild + 'p,
{
  fn from(value: P) -> Self { XMultiChild(Box::new(value)) }
}

// ------ Widget Iterator Conversions ------
impl<'w, I, K> IntoWidgetIter<'w, dyn Iterator<Item = K>> for I
where
  I: IntoIterator<Item: IntoWidget<'w, K>>,
{
  fn into_widget_iter(self) -> impl Iterator<Item = Widget<'w>> {
    self.into_iter().map(IntoWidget::into_widget)
  }
}

impl<P, K> IntoWidgetIter<'static, dyn Pipe<Value = [K]>> for P
where
  P: Pipe<Value: IntoIterator<Item: IntoWidget<'static, K>>>,
{
  fn into_widget_iter(self) -> impl Iterator<Item = Widget<'static>> {
    self.build_multi().into_iter()
  }
}

// for single widget, we ignore the pipe widget with an optional value, because
// it implemented in the before pipe build multi logic.
impl<'w, W: IntoWidget<'w, IntoKind>> IntoWidgetIter<'w, IntoKind> for W {
  fn into_widget_iter(self) -> impl Iterator<Item = Widget<'w>> {
    std::iter::once(self.into_widget())
  }
}
impl<'w, W, K: ?Sized> IntoWidgetIter<'w, OtherWidget<K>> for W
where
  W: IntoWidget<'w, OtherWidget<K>>,
{
  fn into_widget_iter(self) -> impl Iterator<Item = Widget<'w>> {
    std::iter::once(self.into_widget())
  }
}

// ------ MultiChild Implementations ------

impl<'p> MultiChild for XMultiChild<'p> {}

impl<T> MultiChild for T where T: StateReader<Value: MultiChild> {}

impl<P: MultiChild> MultiChild for FatObj<P> {}

impl<P: MultiChild, F: FnOnce() -> P> MultiChild for FnWidget<P, F> {}

/// Macro-generated implementations for pipe types carrying MultiChild values
macro_rules! impl_multi_child_for_pipe {
  (<$($generics:ident),*> , $pipe:ty) => {
    impl<$($generics),*> MultiChild for $pipe
    where
      $pipe: Pipe<Value: Into<XMultiChild<'static>>>,
    {}
  };
}
crate::pipe::iter_all_pipe_type_to_impl!(impl_multi_child_for_pipe);

/// Final conversion from composed MultiPair to XWidget
impl<'w, 'c: 'w, P> RFrom<MultiPair<'c, P>, OtherWidget<dyn Compose>> for Widget<'w>
where
  P: MultiChild + XParent + 'w,
{
  fn r_from(value: MultiPair<'c, P>) -> Self {
    let MultiPair { parent, children } = value;
    parent.x_with_children(children)
  }
}

impl<'p> RFrom<XMultiChild<'p>, OtherWidget<dyn Compose>> for Widget<'p> {
  #[inline]
  fn r_from(value: XMultiChild<'p>) -> Self { value.0.boxed_with_children(vec![]) }
}

impl<'p, P> std::ops::Deref for MultiPair<'p, P> {
  type Target = P;
  fn deref(&self) -> &Self::Target { &self.parent }
}

impl<'p, P> std::ops::DerefMut for MultiPair<'p, P> {
  fn deref_mut(&mut self) -> &mut Self::Target { &mut self.parent }
}
