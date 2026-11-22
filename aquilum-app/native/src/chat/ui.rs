use masonry::core::{NewWidget, PropertySet, Widget};
use masonry::kurbo::Axis;
use masonry::layout::Length;
use masonry::properties::types::CrossAxisAlignment;
use masonry::properties::{Gap, Padding};
use masonry::widgets::Flex;

use super::{ChatBindings, ChatState, Message, Role};
use crate::i18n::t;
use crate::ui::scroll::ScrollArea;
use crate::ui::search;
use crate::ui::settings::controls::InputBox;
use crate::ui::style;
use crate::ui::tokens::{number, size};
use crate::ui::widgets::{Caption, IconButton, Label, Leading, Rule};
use crate::ui::{icons, theme};

pub struct ChatBody {
    pub bindings: ChatBindings,
}

pub fn body(state: &ChatState, enabled: bool, has_base: bool) -> (NewWidget<dyn Widget>, ChatBody) {
    let heading = NewWidget::new(
        Flex::row().with(
            NewWidget::new(
                Label::new(&t("chat.title"), |t| t.text_secondary)
                    .weight(500.0)
                    .leading(Leading::Px((size::FONT_SIZE_14 * number::FONT_LINE_HEIGHT_TIGHT) as f32)),
            ),
            1.0,
        ),
    )
    .with_props(PropertySet::new().with(Gap::new(Length::px(size::SPACE_8))).with(Padding {
        top: Length::px(size::SPACE_8),
        bottom: Length::px(size::SPACE_4),
        left: Length::px(size::SPACE_6),
        right: Length::px(size::SPACE_6),
    }));

    let mut messages = Flex::column().cross_axis_alignment(CrossAxisAlignment::Stretch);
    if state.messages.is_empty() {
        let notice = if !has_base {
            t("chat.noBase")
        } else if !enabled {
            t("chat.disabled")
        } else {
            t("chat.empty")
        };
        messages = messages.with_fixed(
            NewWidget::new(Flex::row().with(NewWidget::new(Caption::new(&notice)), 1.0))
                .with_props(PropertySet::new().with(Padding::all(Length::px(size::SPACE_6)))),
        );
    } else {
        for message in &state.messages {
            messages = messages.with_fixed(bubble(message));
        }
    }
    if state.busy {
        messages = messages.with_fixed(NewWidget::new(Caption::new(&t("chat.busy"))));
    }
    let list = NewWidget::new(messages).with_props(
        PropertySet::new()
            .with(Gap::new(Length::px(size::SPACE_8)))
            .with(Padding::all(Length::px(size::SPACE_8))),
    );

    let (placeholder, field, placeholder_handle) =
        search::field(&state.draft, t("chat.placeholder"), size::INPUT_CONTENT_HEIGHT);
    let send = NewWidget::new(IconButton::new(icons::ARROW_UP, t("chat.send")).inline());
    let send_id = send.id();
    let clear = NewWidget::new(IconButton::new(icons::TRASH, t("chat.clear")).inline());
    let clear_id = clear.id();
    let trailing = NewWidget::new(
        Flex::row()
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .with_fixed(search::after_gap(clear))
            .with_fixed(search::after_gap(send)),
    )
    .with_props(PropertySet::new().with(Gap::new(Length::px(size::SPACE_2))))
    .erased();
    let composer = InputBox::unfilled(InputBox::compact(search::input_row(
        crate::ui::widgets::IconView::new(icons::MESSAGE, size::SIZE_16, size::SIZE_16, 1.4, || theme::current().icon_faint),
        placeholder,
        Some(trailing),
    )));

    let column = Flex::column()
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_fixed(heading)
        .with(NewWidget::new(ScrollArea::new(list)), 1.0)
        .with_fixed(Rule::frame(Axis::Horizontal))
        .with_fixed(
            NewWidget::new(
                Flex::column()
                    .cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .with_fixed(composer),
            )
            .with_props(PropertySet::new().with(Padding::all(Length::px(size::SPACE_8))))
            .with_class(style::SIDEBAR),
        );

    (
        NewWidget::new(column).erased(),
        ChatBody {
            bindings: ChatBindings {
                field,
                placeholder: placeholder_handle,
                send: send_id,
                clear: clear_id,
            },
        },
    )
}

fn bubble(message: &Message) -> NewWidget<dyn Widget> {
    let (paint, weight): (crate::ui::widgets::Paint, f32) = match message.role {
        Role::User => (|t| t.text_link, 600.0),
        Role::Assistant => (|t| t.text_primary, 500.0),
        Role::Tool => (|t| t.text_tertiary, 500.0),
        Role::System => (|t| t.text_secondary, 500.0),
    };
    let label = Label::new(&message.content, paint).weight(weight);
    let pad = match message.role {
        Role::User => Padding {
            top: Length::px(size::SPACE_4),
            bottom: Length::px(size::SPACE_4),
            left: Length::px(size::SPACE_16),
            right: Length::px(size::SPACE_4),
        },
        _ => Padding::all(Length::px(size::SPACE_4)),
    };
    NewWidget::new(Flex::column().with_fixed(NewWidget::new(label)))
        .with_props(PropertySet::new().with(pad))
        .erased()
}
