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
pub struct XMultiChild<'p>(pub(crate) Widget<'p>);

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
  pub fn with_child<'c, K: ?Sized>(self, child: impl IntoWidgetIter<'c, K>) -> MultiPair<'c, P>
  where
    Self: 'c,
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
  P: Into<Parent<'p>> + MultiChild,
{
  fn from(value: P) -> Self { Self(value.into().0) }
}

// ------ Widget Iterator Conversions ------
impl<'w, I, K> IntoWidgetIter<'w, dyn Iterator<Item = K>> for I
where
  I: IntoIterator<Item: IntoWidgetX<'w, K>>,
{
  fn into_widget_iter(self) -> impl Iterator<Item = Widget<'w>> {
    self.into_iter().map(IntoWidgetX::into_widget_x)
  }
}

impl<P, K> IntoWidgetIter<'static, dyn Pipe<Value = [K]>> for P
where
  P: Pipe<Value: IntoIterator<Item: IntoWidgetX<'static, K>>>,
{
  fn into_widget_iter(self) -> impl Iterator<Item = Widget<'static>> {
    self.build_multi().into_iter()
  }
}

// for single widget, we ignore the pipe widget with an optional value, because
// it implemented in the before pipe build multi logic.
impl<'w> IntoWidgetIter<'w, Widget<'w>> for Widget<'w> {
  fn into_widget_iter(self) -> impl Iterator<Item = Widget<'w>> { std::iter::once(self) }
}
impl<'w, W, K> IntoWidgetIter<'w, OtherWidget<K>> for W
where
  W: IntoWidgetX<'w, OtherWidget<K>>,
  K: WidgetKind + ?Sized,
{
  fn into_widget_iter(self) -> impl Iterator<Item = Widget<'w>> {
    std::iter::once(self.into_widget_x())
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
    impl<$($generics),*>  MultiChild for $pipe
    where
      $pipe: Pipe<Value: MultiChild>,
    {}
  };
}
crate::pipe::iter_all_pipe_type_to_impl!(impl_multi_child_for_pipe);

// ------ XWidget Specializations ------

impl<'w> MultiChild for XWidget<'w, OtherWidget<XMultiChild<'w>>> {}

impl<'p> From<XWidget<'p, OtherWidget<XMultiChild<'p>>>> for Parent<'p> {
  fn from(value: XWidget<'p, OtherWidget<XMultiChild<'p>>>) -> Self {
    Parent(value.into_widget_x())
  }
}

/// Final conversion from composed MultiPair to XWidget
impl<'w, 'c: 'w, 'p: 'w, P> From<MultiPair<'c, P>> for XWidget<'w, OtherWidget<dyn Render>>
where
  P: Into<XMultiChild<'p>>,
{
  fn from(value: MultiPair<'w, P>) -> Self {
    let MultiPair { parent, children } = value;
    let w = Widget::new(parent.into().0, children);
    XWidget::new(w)
  }
}

/// Bidirectional conversion between XWidget and XMultiChild
impl<'p> From<XMultiChild<'p>> for XWidget<'p, OtherWidget<XMultiChild<'p>>> {
  #[inline]
  fn from(value: XMultiChild<'p>) -> Self { XWidget::new(value.0) }
}
