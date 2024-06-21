use crate::prelude::*;

#[derive(PartialEq, Clone, Default)]
pub struct MouseHover {
  hover: bool,
}

impl MouseHover {
  pub fn mouse_hover(&self) -> bool { self.hover }
}

impl Declare for MouseHover {
  type Builder = FatObj<()>;
  #[inline]
  fn declarer() -> Self::Builder { FatObj::new(()) }
}

impl ComposeChild for MouseHover {
  type Child<'a> = Widget<'a>;
  fn compose_child<'a>(
    this: impl StateWriter<Value = Self> + 'a, child: Self::Child<'a>,
  ) -> impl FnWidget + 'a {
    fn_widget! {
      @ $child {
        on_pointer_enter: move |_| $this.write().hover = true,
        on_pointer_leave: move |_| $this.write().hover = false,
      }
    }
  }
}
