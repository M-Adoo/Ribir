use child_convert::INTO_CONVERT;

use super::*;
// Template use to construct child of a widget.
pub trait Template: Sized {
  type Builder: TemplateBuilder;
  fn builder() -> Self::Builder;
}

pub trait TemplateBuilder: Sized {
  type Target;
  fn build_tml(self) -> Self::Target;
}

macro_rules! stateless_with_child {
  ($m:expr) => {
    impl<T, C> WithChild<C, { 10 + $m }> for T
    where
      T: ComposeChild,
      State<T>: WithChild<C, $m>,
    {
      type Target = <State<T> as WithChild<C, $m>>::Target;

      #[track_caller]
      fn with_child(self, child: C, ctx: &BuildCtx) -> Self::Target {
        State::value(self).with_child(child, ctx)
      }
    }
  };
}

macro_rules! writer_with_child {
  ($($m:ident),*) => {
    $(
      impl<T, C, Child> WithChild<C, { 200 + $m }> for T
      where
        T: StateWriter,
        T::Value: ComposeChild<Child=Child>,
        C: IntoChild<Child, $m>,
      {
        type Target = Pair<T, Child>;

        #[track_caller]
        fn with_child(self, c: C, ctx: &BuildCtx) -> Self::Target {
          Pair { parent: self, child: c.into_child(ctx) }
        }
      }

      stateless_with_child!({ 200 + $m });
    )*
  };
}

macro_rules! with_template_child {
  ($($m:ident),*) => {
    $(
      impl<W, C, Child> WithChild<C, { 220 + $m }> for W
      where
        W: StateWriter,
        W::Value: ComposeChild<Child = Child>,
        Child: Template,
        Child::Builder: WithChild<C, $m, Target = Child::Builder>,
      {
        type Target = Pair<W, Child::Builder>;

        #[inline]
        #[track_caller]
        fn with_child(self, c: C, ctx: &BuildCtx) -> Self::Target {
          let builder = Child::builder();
          let child = builder.with_child(c, ctx);
          Pair { parent: self, child }
        }
      }

      stateless_with_child!({ 220 + $m });
    )*
  };
}

macro_rules! option_template_with_child {
  ($($m:ident),*) => {
    $(
      impl<W, C, Child> WithChild<C, {240 + $m}> for W
      where
        W: StateWriter,
        W::Value: ComposeChild<Child = Option<Child>>,
        Child: Template,
        Child::Builder: WithChild<C, $m, Target = Child::Builder>,
      {
        type Target = Pair<W, Child::Builder>;
        #[track_caller]
        fn with_child(self, c: C, ctx: &BuildCtx) -> Self::Target {
          let builder = Child::builder();
          let child = builder.with_child(c, ctx);
          Pair { parent: self, child }
        }
      }

      stateless_with_child!({ 240 + $m });
    )*
  };
}

writer_with_child!(INTO_CONVERT, RENDER, COMPOSE, COMPOSE_CHILD, FN);
with_template_child!(INTO_CONVERT, RENDER, COMPOSE, COMPOSE_CHILD, FN);
option_template_with_child!(INTO_CONVERT, RENDER, COMPOSE, COMPOSE_CHILD, FN);

impl<const M: usize, W, C1, C2> WithChild<C2, M> for Pair<W, C1>
where
  C1: WithChild<C2, M>,
{
  type Target = Pair<W, C1::Target>;
  #[track_caller]
  fn with_child(self, c: C2, ctx: &BuildCtx) -> Self::Target {
    let Pair { parent: widget, child } = self;
    Pair { parent: widget, child: child.with_child(c, ctx) }
  }
}

/// Trait specify what child a compose child widget can have, and the target
/// type after widget compose its child.
pub trait ComposeWithChild<C, M> {
  type Target;
  fn with_child(self, child: C, ctx: &BuildCtx) -> Self::Target;
}

impl<W, C, Child> WidgetBuilder for Pair<W, C>
where
  W: StateWriter,
  W::Value: ComposeChild<Child = Child>,
  Child: From<C>,
{
  #[inline]
  fn build(self, ctx: &BuildCtx) -> Widget {
    let Self { parent, child } = self;
    ComposeChild::compose_child(parent, child.into()).build(ctx)
  }
}

// impl Vec<T> as Template

impl<T> Template for Vec<T> {
  type Builder = Self;
  #[inline]
  fn builder() -> Self::Builder { vec![] }
}

impl<T> Template for BoxPipe<Vec<T>> {
  type Builder = Vec<T>;
  #[inline]
  fn builder() -> Self::Builder { vec![] }
}

impl<T> TemplateBuilder for Vec<T> {
  type Target = Self;
  #[inline]
  fn build_tml(self) -> Self::Target { self }
}

macro_rules! vec_with_child {
  ($($m:ident),*) => {
    $(
      impl<C, T> WithChild<C, $m> for Vec<T>
      where
        C: IntoChild<T, $m>,
      {
        type Target = Self;

        #[inline]
        #[track_caller]
        fn with_child(mut self, child: C, ctx: &BuildCtx) -> Self::Target {
          self.push(child.into_child(ctx));
          self
        }
      }
    )*
  };
}

macro_rules! vec_with_iter_child {
  ($($m:ident),*) => {
    $(
      impl<C, T> WithChild<C, {10 + $m}> for Vec<T>
      where
        C: IntoIterator,
        C::Item: IntoChild<T, $m>,
      {
        type Target = Self;

        #[inline]
        #[track_caller]
        fn with_child(mut self, child: C, ctx: &BuildCtx) -> Self::Target {
          self.extend(
            child
              .into_iter()
              .map(|v| v.into_child(ctx)),
          );
          self
        }
      }
    )*
  };
}

vec_with_child!(INTO_CONVERT, RENDER, COMPOSE, COMPOSE_CHILD, FN);
vec_with_iter_child!(INTO_CONVERT, RENDER, COMPOSE, COMPOSE_CHILD, FN);

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{prelude::*, test_helper::MockBox};

  #[derive(Template)]
  struct PTml {
    _child: CTml,
  }

  #[derive(Template)]
  enum CTml {
    Void(Void),
  }

  struct P;

  impl ComposeChild for P {
    type Child = PTml;
    fn compose_child(_: impl StateWriter<Value = Self>, _: Self::Child) -> impl WidgetBuilder {
      fn_widget!(Void)
    }
  }

  #[derive(Declare)]
  struct X;

  impl ComposeChild for X {
    type Child = Widget;

    fn compose_child(_: impl StateWriter<Value = Self>, _: Self::Child) -> impl WidgetBuilder {
      fn_widget!(Void)
    }
  }

  #[test]
  fn template_fill_template() { let _ = |ctx| P.with_child(Void, ctx).build(ctx); }

  #[test]
  fn pair_compose_child() {
    let _ = |ctx| -> Widget {
      MockBox { size: ZERO_SIZE }
        .with_child(X.with_child(Void {}, ctx), ctx)
        .build(ctx)
    };
  }

  #[derive(Declare)]
  struct PipeParent;

  impl ComposeChild for PipeParent {
    type Child = BoxPipe<usize>;

    fn compose_child(_: impl StateWriter<Value = Self>, _: Self::Child) -> impl WidgetBuilder {
      fn_widget!(Void)
    }
  }

  #[test]
  fn compose_pipe_child() {
    let _value_child = fn_widget! {
      @PipeParent {  @ { 0 } }
    };

    let _pipe_child = fn_widget! {
      let state = State::value(0);
      @PipeParent {  @ { pipe!(*$state) } }
    };
  }
}
