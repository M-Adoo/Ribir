use smallvec::SmallVec;

use super::*;
use crate::pipe::InnerPipe;

pub struct MultiPair {
  pub parent: Widget,
  pub children: SmallVec<[Widget; 1]>,
}

macro_rules! directly_with_child {
  ($m:expr) => {
    impl<C, T> WithChild<C, $m> for T
    where
      T: MultiChild + IntoWidget<RENDER>,
      MultiPair: WithChild<C, $m>,
    {
      type Target = <MultiPair as WithChild<C, $m>>::Target;

      #[inline]
      fn with_child(self, child: C, ctx: &BuildCtx) -> Self::Target {
        MultiPair { parent: self.into_widget(ctx), children: SmallVec::new() }
          .with_child(child, ctx)
      }
    }
  };
}

macro_rules! impl_widget_child {
  ($($m: ident),*) => {
    $(
      impl< C:IntoWidget<$m>> WithChild<C, { 100 + $m }> for MultiPair
      {
        type Target = Self;
        #[inline]
        fn with_child(mut self, child: C, ctx: &BuildCtx) -> Self::Target {
          self.children.push(child.into_widget(ctx));
          self
        }
      }

      directly_with_child!({ 100 + $m });
    )*
  };
}

macro_rules! impl_iter_widget_child {
  ($($m: ident), *) => {
    $(
      impl<C> WithChild<C, { 110 + $m }> for MultiPair
      where
        C:IntoIterator,
        C::Item: IntoWidget<$m>,
      {
        type Target = Self;
        #[inline]
        fn with_child(mut self, child: C, ctx: &BuildCtx) -> Self::Target {
          self.children.extend(child.into_iter().map(|w| w.into_widget(ctx)));
          self
        }
      }

      directly_with_child!({ 110 + $m });
    )*
  };
}

macro_rules! impl_pipe_iter_widget_child {
  ($($m: ident), *) => {
    $(
      impl<C, V> WithChild<C, { 120 + $m }> for MultiPair
      where
        C:InnerPipe<Value=V>,
        V:IntoIterator,
        V::Item: IntoWidget<$m>,
      {
        type Target = Self;
        fn with_child(mut self, child: C, ctx: &BuildCtx) -> Self::Target {
          self.children.extend(child.build_multi(ctx));
          self
        }
      }

      directly_with_child!({ 120 + $m });
    )*
  };
}

// Choose `IntoWidget` for child widgets instead of `IntoChild<Widget>`. This is
// because `IntoChild<Widget>` may lead
// `Pipe<Value = Option<impl IntoWidget>>` has two implementations:
//
// - As a single widget child, satisfy the `IntoChild<Widget>` requirement,
//   albeit not `IntoWidget`.
// - As a `Pipe` that facilitates iteration over multiple widgets.
impl_widget_child!(COMPOSE, RENDER, COMPOSE_CHILD, FN);
impl_iter_widget_child!(COMPOSE, RENDER, COMPOSE_CHILD, FN);
impl_pipe_iter_widget_child!(COMPOSE, RENDER, COMPOSE_CHILD, FN);

impl WidgetBuilder for MultiPair {
  fn build(self, ctx: &BuildCtx) -> Widget { self.into_widget_strict(ctx) }
}

impl IntoWidgetStrict<COMPOSE_CHILD> for MultiPair {
  fn into_widget_strict(self, ctx: &BuildCtx) -> Widget {
    let MultiPair { parent, children } = self;
    let leaf = parent.id().single_leaf(&ctx.tree.borrow().arena);
    for c in children {
      ctx.append_child(leaf, c);
    }
    parent
  }
}
