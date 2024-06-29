use child_convert::INTO_CONVERT;

use super::*;

macro_rules! parent_with_option_child {
  ($m1:ident, $m2:expr) => {
    impl<P, C> WithChild<Option<C>, { 10 + $m2 }> for P
    where
      P: WithChild<C, $m2, Target = Widget>,
      P: IntoChild<Widget, RENDER>,
    {
      type Target = Widget;
      #[track_caller]
      fn with_child(self, child: Option<C>, ctx: &BuildCtx) -> Self::Target {
        if let Some(child) = child { self.with_child(child, ctx) } else { self.into_child(ctx) }
      }
    }
  };
}

macro_rules! option_parent_with_child {
  ($m1:ident, $m2:expr) => {
    impl<P, C> WithChild<C, { 20 + $m2 }> for Option<P>
    where
      P: WithChild<C, $m2, Target = Widget>,
      C: IntoChild<Widget, $m1>,
    {
      type Target = Widget;
      #[track_caller]
      fn with_child(self, child: C, ctx: &BuildCtx) -> Self::Target {
        if let Some(parent) = self { parent.with_child(child, ctx) } else { child.into_child(ctx) }
      }
    }
  };
}

macro_rules! parent_with_child {
  ($($m: ident),*) => {
    $(
      impl<P, C> WithChild<C, $m> for P
      where
        P: SingleChild + IntoChild<Widget, RENDER>,
        C: IntoChild<Widget, $m>
      {
        type Target = Widget;
        #[track_caller]
        fn with_child(self, child: C, ctx: &BuildCtx) -> Self::Target {
          let p = self.into_child(ctx);
          single_parent_compose_child(p, child.into_child(ctx), ctx)
        }
      }

      parent_with_option_child!($m, $m);
      option_parent_with_child!($m, $m);
    )*
  };
}

parent_with_child!(INTO_CONVERT, RENDER, COMPOSE, COMPOSE_CHILD, FN);

fn single_parent_compose_child(p: Widget, c: Widget, ctx: &BuildCtx) -> Widget {
  let c = c.into_child(ctx);
  let p_leaf = p.id().single_leaf(&ctx.tree.borrow().arena);
  ctx.append_child(p_leaf, c);
  p
}
#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_helper::MockBox;

  #[test]
  fn pair_with_child() {
    let mock_box = MockBox { size: ZERO_SIZE };
    let _ = |ctx| -> Widget {
      mock_box
        .clone()
        .with_child(mock_box.clone().with_child(mock_box, ctx), ctx)
        .build(ctx)
    };
  }

  #[test]
  fn fix_mock_box_compose_pipe_option_widget() {
    fn _x(w: BoxPipe<Option<Widget>>, ctx: &BuildCtx) {
      MockBox { size: ZERO_SIZE }.with_child(w.into_pipe(), ctx);
    }
  }
}
