#![allow(clippy::needless_lifetimes)]
#![cfg_attr(feature = "nightly", feature(closure_track_caller))]
#[macro_use]
extern crate bitflags;

pub mod animation;
pub mod builtin_widgets;
pub mod clipboard;
mod context;
pub mod data_widget;
pub mod declare;
pub mod events;
pub mod local_sender;
pub mod pipe;
pub(crate) mod render_helper;
mod state;
pub mod ticker;
pub mod timer;
pub mod widget;
pub mod widget_children;
pub(crate) mod widget_tree;
pub mod window;
pub use rxrust;
pub mod overlay;
pub mod query;
pub mod wrap_render;
pub mod prelude {
  pub use log;
  #[doc(no_inline)]
  pub use ribir_algo::*;
  pub use ribir_geom::*;
  #[doc(no_inline)]
  pub use ribir_macros::*;
  #[doc(no_inline)]
  pub use ribir_painter::*;
  #[doc(hidden)]
  pub use rxrust::prelude::*;
  pub use smallvec;

  #[doc(no_inline)]
  pub use crate::builtin_widgets::*;
  #[doc(no_inline)]
  pub use crate::context::*;
  #[doc(no_inline)]
  pub use crate::declare::*;
  #[doc(no_inline)]
  pub use crate::events::*;
  #[doc(no_inline)]
  pub use crate::overlay::{AutoClosePolicy, Overlay, OverlayStyle};
  #[doc(no_inline)]
  pub use crate::pipe::{BoxPipe, FinalChain, MapPipe, ModifiesPipe, Pipe};
  #[doc(no_inline)]
  pub use crate::state::*;
  #[doc(no_inline)]
  pub use crate::widget;
  #[doc(no_inline)]
  pub use crate::widget::*;
  #[doc(no_inline)]
  pub use crate::widget_children::*;
  #[doc(no_inline)]
  pub use crate::widget_tree::{BoxClamp, LayoutInfo, TrackId, WidgetId};
  #[doc(no_inline)]
  pub use crate::window::Window;
  pub use crate::{
    animation::*,
    class_names, multi_class_impl, providers,
    query::*,
    style_class,
    ticker::{Duration, Instant},
  };
}

pub mod test_helper;
