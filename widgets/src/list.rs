use ribir_core::prelude::*;

use crate::prelude::*;

/// Lists usage
///
/// use `ListItem` must have `HeadlineText`, other like `SupportingText`,
/// `Leading`, and `Trailing` are optional.
///
/// # Example
///
/// ## single headline text
/// ```
/// # use ribir_core::prelude::*;
/// # use ribir_widgets::prelude::*;
///
/// // only single headline text
/// fn_widget! {
///   @Lists {
///     @ListItem {
///       @{ HeadlineText(Label::new("One line list item")) }
///     }
///   }
/// };
/// ```
///
/// ## headline text and supporting text
/// ```
/// # use ribir_core::prelude::*;
/// # use ribir_widgets::prelude::*;
///
/// // single headline text and supporting text
/// fn_widget! {
///   @Lists {
///     @ListItem {
///       @ { HeadlineText(Label::new("headline text")) }
///       @ { SupportingText(Label::new("supporting text")) }
///     }
///   }
/// };
/// ```
///
/// ## use leading
/// ```
/// # use ribir_core::prelude::*;
/// # use ribir_widgets::prelude::*;
///
/// fn_widget! {
///   @Lists {
///     // use leading icon
///     @ListItem {
///       @Leading::new(EdgeWidget::Icon(svgs::CHECK_BOX_OUTLINE_BLANK.into_widget()))
///       @HeadlineText(Label::new("headline text"))
///     }
///     // use leading label
///     @ListItem {
///       @Leading::new(EdgeWidget::Text(Label::new("A")))
///       @HeadlineText(Label::new("headline text"))
///     }
///     // use leading custom widget
///     @ListItem {
///       @Leading::new(
///         EdgeWidget::Custom(
///           @CustomEdgeWidget(
///              @Container {
///                size: Size::splat(40.),
///                background: Color::YELLOW,
///              }.into_widget()
///           )
///         )
///       )
///     }
///   }
/// };
/// ```
///
/// ## use trailing
/// ```
/// # use ribir_core::prelude::*;
/// # use ribir_widgets::prelude::*;
///
/// fn_widget! {
///   @Lists {
///     // use trailing icon
///     @ListItem {
///       @HeadlineText(Label::new("headline text"))
///       @Trailing::new(EdgeWidget::Icon(svgs::CHECK_BOX_OUTLINE_BLANK.into_widget()))
///     }
///     // use trailing label
///     @ListItem {
///       @HeadlineText(Label::new("headline text"))
///       @Trailing::new(EdgeWidget::Text(Label::new("A")))
///     }
///     // use trailing custom widget
///     @ListItem {
///       @HeadlineText(Label::new("headline text"))
///       @Trailing::new(
///         EdgeWidget::Custom(
///           @CustomEdgeWidget(
///             @Container {
///               size: Size::splat(40.),
///               background: Color::YELLOW,
///             }.into_widget()
///           )
///         )
///       )
///     }
///   }
/// };
/// ```
///
/// ## use `Divider` split list item
/// ```
/// # use ribir_core::prelude::*;
/// # use ribir_widgets::prelude::*;
///
/// fn_widget! {
///   @Lists {
///     @ListItem {
///       @ { HeadlineText(Label::new("One line list item")) }
///     }
///     @Divider {}
///     @ListItem {
///       @ { HeadlineText(Label::new("One line list item")) }
///     }
///   }
/// };
/// ```
#[derive(Declare)]
pub struct List;

class_names! {
  /// The root container class name for the `List` widget.
  LIST,
  /// The class name for the container of a `ListItem`.
  LIST_ITEM_CONTAINER,
  /// The class name for the container of a `ListItem` if it is interactive.
  LIST_ITEM_INTERACTIVE_CONTAINER,
  /// The class name for the content section of a `ListItem`.
  LIST_ITEM_CONTENT,
  /// The class name for the headline section of a `ListItem`.
  LIST_ITEM_HEADLINE,
  /// The class name for the supporting text section of a `ListItem`.
  LIST_ITEM_SUPPORTING,
  /// The class name for the trailing supporting text section of a `ListItem`.
  LIST_ITEM_TRAILING_SUPPORTING,
  /// The class name for the image within a `ListItem`.
  LIST_ITEM_IMG,
  /// The class name for the thumbnail within a `ListItem`.
  LIST_ITEM_THUMB_NAIL,
  /// The class name for the leading widget of a `ListItem`.
  LIST_ITEM_LEADING,
  /// The class name for the trailing widget of a `ListItem`.
  LIST_ITEM_TRAILING
}

#[derive(Declare, Clone)]
pub struct ListItem {
  #[declare(default = 1usize)]
  pub supporting_lines: usize,
  #[declare(default = false)]
  pub interactive: bool,
}

/// A theme provider that controls the vertical alignment of a `ListItem`'s
/// leading, content, and trailing sections.
#[derive(Default, Clone)]
pub struct ListItemAlignItems(pub Align);

#[derive(Template)]
pub struct ListItemHeadline(TextInit);

#[derive(Template)]
pub struct ListItemSupporting(TextInit);

#[derive(Template)]
pub struct ListItemTrailingSupporting(TextInit);

#[simple_declare]
pub struct ListItemImg;

#[simple_declare]
pub struct ListItemThumbNail;

#[derive(Template)]
pub struct ListItemChildren<'w> {
  headline: ListItemHeadline,
  supporting: Option<ListItemSupporting>,
  trailing_supporting: Option<ListItemTrailingSupporting>,
  leading: Option<Widget<'w>>,
  trailing: Option<Trailing<Widget<'w>>>,
}

pub struct ListItemStructInfo {
  pub supporting: bool,
  pub trailing_supporting: bool,
  pub leading: bool,
  pub trailing: bool,
}

impl<'c> ComposeChild<'c> for List {
  type Child = Vec<Widget<'c>>;

  fn compose_child(_: impl StateWriter<Value = Self>, child: Self::Child) -> Widget<'c> {
    self::column! { class: LIST, @ { child } }.into_widget()
  }
}

impl<'c> ComposeChild<'c> for ListItem {
  type Child = ListItemChildren<'c>;

  fn compose_child(this: impl StateWriter<Value = Self>, child: Self::Child) -> Widget<'c> {
    let this = Variant::from_watcher(this);
    let ListItemChildren { headline, supporting, trailing_supporting, leading, trailing } = child;
    let item_struct_info = ListItemStructInfo {
      supporting: supporting.is_some(),
      trailing_supporting: trailing_supporting.is_some(),
      leading: leading.is_some(),
      trailing: trailing.is_some(),
    };

    let headline = text! { class: LIST_ITEM_HEADLINE, text: headline.0 };
    let line_num_var = this.clone().map(|t| t.supporting_lines as f32);
    let content = if let Some(supporting) = supporting {
      self::column! {
        class: LIST_ITEM_CONTENT,
        align_items: Align::Stretch,
        @ { headline }
        @TextClamp {
          class: LIST_ITEM_SUPPORTING,
          rows: line_num_var,
          @Text { text: supporting.0 }
        }
      }
      .into_widget()
    } else {
      class! {
        class: LIST_ITEM_CONTENT,
        @ { headline }
      }
      .into_widget()
    };

    let trailing_supporting = trailing_supporting.map(|s| {
      text_clamp! {
        class: LIST_ITEM_TRAILING_SUPPORTING,
        rows: Some(1.),
        @Text{ text: s.0 }
      }
    });

    let leading_widget = leading.map(|l| {
      class! { class: LIST_ITEM_LEADING, @ { l } }
    });
    let trailing_widget = trailing.map(|t| {
      class! { class: LIST_ITEM_TRAILING, @ { t.unwrap() } }
    });

    providers! {
      providers: [Provider::new(item_struct_info)],
      @Class {
        class: this.map(|t| t.interactive.then_some(LIST_ITEM_INTERACTIVE_CONTAINER)),
        @Class {
          class: LIST_ITEM_CONTAINER,
          @row! {
            align_items: ListItemAlignItems::get_align(BuildCtx::get()),
            @ { leading_widget }
            @Expanded { defer_alloc: true, @ { content } }
            @ { trailing_supporting }
            @ { trailing_widget }
          }
        }
      }
    }
    .into_widget()
  }
}

impl<'c> ComposeChild<'c> for ListItemImg {
  type Child = Widget<'c>;

  fn compose_child(_: impl StateWriter<Value = Self>, child: Self::Child) -> Widget<'c> {
    class! { class: LIST_ITEM_IMG, @ { child } }.into_widget()
  }
}

impl<'c> ComposeChild<'c> for ListItemThumbNail {
  type Child = Widget<'c>;

  fn compose_child(_: impl StateWriter<Value = Self>, child: Self::Child) -> Widget<'c> {
    class! { class: LIST_ITEM_THUMB_NAIL, @ { child } }.into_widget()
  }
}

impl ListItemAlignItems {
  pub fn get_align(ctx: &BuildCtx) -> DeclareInit<Align> {
    Variant::<Self>::new_or_default(ctx)
      .map(|v| v.0)
      .declare_into()
  }
}

// todo: Select item style
// todo: color
