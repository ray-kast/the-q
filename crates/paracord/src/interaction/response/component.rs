use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    marker::PhantomData,
};

use qcore::{
    build_range::BuildRange,
    build_with::{BuildWith, BuilderHelpers},
    builder,
};
use serenity::{
    all::{ChannelId, RoleId, UserId},
    builder::{
        CreateActionRow, CreateButton, CreateInputText, CreateInteractionResponseFollowup,
        CreateInteractionResponseMessage, CreateModal, CreateSelectMenu, CreateSelectMenuKind,
        CreateSelectMenuOption, EditInteractionResponse,
    },
    model::{
        application::{ButtonStyle as ButtonStyleModel, InputTextStyle},
        channel::{ChannelType, ReactionType},
    },
};
use url::Url;

use super::{super::rpc::ComponentId, id, Prepare};

/// A set of components to attach to a message
#[derive(Debug)]
#[repr(transparent)]
pub struct Components<R>(pub(super) Vec<R>);

impl<R> Default for Components<R> {
    #[inline]
    fn default() -> Self { Self(vec![]) }
}

macro_rules! build_components {
    ($self:expr, $builder:expr) => {{
        let Components(rows) = $self;
        $builder.components(rows.into_iter().map(Into::into).collect())
    }};
}

impl<I: ComponentId> Components<MessageComponent<I, id::Error>> {
    fn menu_parts(
        &mut self,
        payload: I::Payload,
        ty: MenuType<I, id::Error>,
        placeholder: impl Into<Option<String>>,
        count: impl BuildRange<u8>,
        disabled: bool,
    ) {
        let (min_count, max_count) = count.build_range().into_inner();
        self.0.push(MessageComponent::Menu(Menu {
            id: id::write(&I::from_parts(payload)),
            ty,
            placeholder: placeholder.into(),
            min_count: min_count.unwrap_or(0),
            max_count, // max allowed
            disabled,
            rpc_id: PhantomData,
        }));
    }
}

#[builder(trait_name = ComponentsExt)]
/// Helper methods for mutating [`Components`]
impl<R> Components<R> {
    /// Add a new action row to the component set
    pub fn row(&mut self, row: R) { self.0.push(row); }
}

/// Helper methods for mutating [`Components`] for messages
#[builder(trait_name = MessageComponents)]
impl<I: ComponentId> Components<MessageComponent<I, id::Error>> {
    /// Create a row with buttons using the given closure
    pub fn buttons(&mut self, f: impl FnOnce(ButtonsBuilder<I>) -> ButtonsBuilder<I>) {
        self.0
            .push(MessageComponent::Buttons(f(ButtonsBuilder::default()).0));
    }

    /// Add a new row with a string dropdown menu
    pub fn menu<J: Into<MenuItem>>(
        &mut self,
        payload: I::Payload,
        placeholder: impl Into<Option<String>>,
        count: impl BuildRange<u8>,
        disabled: bool,
        default: impl IntoIterator<Item = usize>,
        options: impl IntoIterator<Item = (I::Payload, J)>,
    ) {
        let (items, order) = options.into_iter().fold(
            (HashMap::new(), vec![]),
            |(mut items, mut order), (payload, item)| {
                let id = id::write(&I::from_parts(payload));

                if let Ok(ref id) = id {
                    assert!(items.insert(id.clone(), item.into()).is_none());
                }
                order.push(id);

                (items, order)
            },
        );

        let default: HashSet<_> = default.into_iter().collect();
        assert!(default.iter().all(|d| order.len() > *d));

        self.menu_parts(
            payload,
            MenuType::String {
                items,
                order,
                default,
                rpc_id: PhantomData,
            },
            placeholder,
            count,
            disabled,
        );
    }

    /// Add a new row with a user handle dropdown menu
    #[inline]
    pub fn user_menu(
        &mut self,
        payload: I::Payload,
        placeholder: impl Into<Option<String>>,
        count: impl BuildRange<u8>,
        disabled: bool,
        default: impl IntoIterator<Item = UserId>,
    ) {
        self.menu_parts(
            payload,
            MenuType::User(default.into_iter().collect()),
            placeholder,
            count,
            disabled,
        );
    }

    /// Add a new row with a role handle dropdown menu
    #[inline]
    pub fn role_menu(
        &mut self,
        payload: I::Payload,
        placeholder: impl Into<Option<String>>,
        count: impl BuildRange<u8>,
        disabled: bool,
        default: impl IntoIterator<Item = RoleId>,
    ) {
        self.menu_parts(
            payload,
            MenuType::Role(default.into_iter().collect()),
            placeholder,
            count,
            disabled,
        );
    }

    /// Add a new row with a user or role handle dropdown menu
    #[inline]
    pub fn mention_menu(
        &mut self,
        payload: I::Payload,
        placeholder: impl Into<Option<String>>,
        count: impl BuildRange<u8>,
        disabled: bool,
        default_user: impl IntoIterator<Item = UserId>,
        default_role: impl IntoIterator<Item = RoleId>,
    ) {
        self.menu_parts(
            payload,
            MenuType::Mention(
                default_user.into_iter().collect(),
                default_role.into_iter().collect(),
            ),
            placeholder,
            count,
            disabled,
        );
    }

    /// Add a new row with a channel dropdown menu
    #[inline]
    pub fn channel_menu(
        &mut self,
        payload: I::Payload,
        placeholder: impl Into<Option<String>>,
        count: impl BuildRange<u8>,
        disabled: bool,
        tys: impl IntoIterator<Item = ChannelType>,
        default: impl IntoIterator<Item = ChannelId>,
    ) {
        self.menu_parts(
            payload,
            MenuType::Channel(tys.into_iter().collect(), default.into_iter().collect()),
            placeholder,
            count,
            disabled,
        );
    }
}

/// Helper methods for mutating [`Components`] for modals
#[builder(trait_name = ModalComponents)]
impl<I: ComponentId> Components<TextInput<I, id::Error>> {
    /// Create a row with a short textbox using the given closure
    #[inline]
    pub fn text_short(
        &mut self,
        payload: I::Payload,
        label: impl Into<String>,
        f: impl FnOnce(TextInput<I, id::Error>) -> TextInput<I, id::Error>,
    ) {
        self.0.push(f(TextInput::short(payload, label)));
    }

    /// Create a row with a paragraph textbox using the given closure
    #[inline]
    pub fn text_long(
        &mut self,
        payload: I::Payload,
        label: impl Into<String>,
        f: impl FnOnce(TextInput<I, id::Error>) -> TextInput<I, id::Error>,
    ) {
        self.0.push(f(TextInput::long(payload, label)));
    }
}

impl<R: Prepare> Prepare for Components<R> {
    type Error = R::Error;
    type Output = Components<R::Output>;

    fn prepare(self) -> Result<Self::Output, Self::Error> {
        self.0
            .into_iter()
            .map(R::prepare)
            .collect::<Result<Vec<_>, _>>()
            .map(Components)
    }
}

impl<R: Into<CreateActionRow>> BuildWith<Components<R>> for CreateInteractionResponseMessage {
    #[inline]
    fn build_with(self, value: Components<R>) -> Self { build_components!(value, self) }
}

impl<R: Into<CreateActionRow>> BuildWith<Components<R>> for EditInteractionResponse {
    #[inline]
    fn build_with(self, value: Components<R>) -> Self { build_components!(value, self) }
}

impl<R: Into<CreateActionRow>> BuildWith<Components<R>> for CreateInteractionResponseFollowup {
    #[inline]
    fn build_with(self, value: Components<R>) -> Self { build_components!(value, self) }
}

impl<R: Into<CreateActionRow>> BuildWith<Components<R>> for CreateModal {
    #[inline]
    fn build_with(self, value: Components<R>) -> Self { build_components!(value, self) }
}

/// Helper for building a row of buttons
#[derive(Debug)]
#[repr(transparent)]
pub struct ButtonsBuilder<I>(Vec<Button<I, id::Error>>);

impl<I> Default for ButtonsBuilder<I> {
    #[inline]
    fn default() -> Self { Self(vec![]) }
}

#[builder(trait_name = ButtonsBuilderExt)]
/// Helper methods for mutating an
/// [`ActionRow`](serenity::model::application::ActionRow) for messages
impl<I: ComponentId> ButtonsBuilder<I> {
    /// Add a button to this row by value
    pub fn push(&mut self, btn: Button<I, id::Error>) { self.0.push(btn); }

    /// Add a button to this row
    pub fn button(
        &mut self,
        payload: I::Payload,
        style: ButtonStyle,
        label: impl Into<ButtonLabel>,
        disabled: bool,
    ) {
        self.0.push(Button {
            label: label.into(),
            ty: ButtonType::Custom {
                id: id::write(&I::from_parts(payload)),
                style,
                rpc_id: PhantomData,
            },
            disabled,
        });
    }

    /// Add a new link-style button to this row
    pub fn link(&mut self, url: impl Into<Url>, label: impl Into<ButtonLabel>, disabled: bool) {
        self.0.push(Button {
            ty: ButtonType::Link(url.into()),
            label: label.into(),
            disabled,
        });
    }
}

/// A single row of components that are valid inside a message
#[derive(Debug)]
pub enum MessageComponent<I, E> {
    /// A row of buttons
    Buttons(Vec<Button<I, E>>),
    /// A single menu occupying a full row
    Menu(Menu<I, E>),
}

impl<I, E> Prepare for MessageComponent<I, E> {
    type Error = E;
    type Output = MessageComponent<I, Infallible>;

    fn prepare(self) -> Result<Self::Output, Self::Error> {
        match self {
            Self::Buttons(b) => b
                .into_iter()
                .map(Prepare::prepare)
                .collect::<Result<Vec<_>, _>>()
                .map(MessageComponent::Buttons),
            Self::Menu(m) => m.prepare().map(MessageComponent::Menu),
        }
    }
}

impl<I> From<MessageComponent<I, Infallible>> for CreateActionRow {
    fn from(value: MessageComponent<I, Infallible>) -> Self {
        match value {
            MessageComponent::Buttons(b) => Self::Buttons(b.into_iter().map(Into::into).collect()),
            MessageComponent::Menu(m) => Self::SelectMenu(m.into()),
        }
    }
}

/// A single button component
#[derive(Debug)]
pub struct Button<I, E> {
    ty: ButtonType<I, E>,
    label: ButtonLabel,
    disabled: bool,
}

impl<I, E> Prepare for Button<I, E> {
    type Error = E;
    type Output = Button<I, Infallible>;

    #[inline]
    fn prepare(self) -> Result<Self::Output, Self::Error> {
        let Self {
            ty,
            label,
            disabled,
        } = self;
        Ok(Button {
            ty: ty.prepare()?,
            label,
            disabled,
        })
    }
}

impl<I> From<Button<I, Infallible>> for CreateButton {
    fn from(value: Button<I, Infallible>) -> Self {
        let Button {
            ty,
            label,
            disabled,
        } = value;
        match ty {
            ButtonType::Link(l) => CreateButton::new_link(l),
            ButtonType::Custom {
                id,
                style,
                rpc_id: _,
            } => CreateButton::new(id.unwrap_or_else(|_| unreachable!()).to_string())
                .style(style.into()),
        }
        .build_with(label)
        .disabled(disabled)
    }
}

/// The label of a button, composed of text and/or an emoji
#[derive(Debug)]
pub enum ButtonLabel {
    /// A text label with an optional emoji
    Text(Option<ReactionType>, String),
    /// An emoji-only label
    Emoji(ReactionType),
}

impl From<String> for ButtonLabel {
    fn from(text: String) -> Self { Self::Text(None, text) }
}

impl From<&str> for ButtonLabel {
    fn from(text: &str) -> Self { Self::Text(None, text.into()) }
}

impl From<ReactionType> for ButtonLabel {
    fn from(emoji: ReactionType) -> Self { Self::Emoji(emoji) }
}

impl From<(ReactionType, String)> for ButtonLabel {
    fn from((emoji, text): (ReactionType, String)) -> Self { Self::Text(Some(emoji), text) }
}

impl From<(Option<ReactionType>, String)> for ButtonLabel {
    fn from((emoji, text): (Option<ReactionType>, String)) -> Self { Self::Text(emoji, text) }
}

impl BuildWith<ButtonLabel> for CreateButton {
    fn build_with(self, value: ButtonLabel) -> Self {
        match value {
            ButtonLabel::Text(e, t) => self.fold_opt(e, Self::emoji).label(t),
            ButtonLabel::Emoji(e) => self.emoji(e),
        }
    }
}

/// The type of a button component
#[derive(Debug)]
pub enum ButtonType<I, E> {
    /// A link-style button
    Link(Url),
    // TODO: make this a struct?
    /// A non-link button
    Custom {
        /// Button ID for callbacks
        id: Result<id::Id<'static>, E>,
        /// Button style
        style: ButtonStyle,
        /// RPC component ID info
        rpc_id: PhantomData<fn(I)>,
    },
}

impl<I, E> Prepare for ButtonType<I, E> {
    type Error = E;
    type Output = ButtonType<I, Infallible>;

    fn prepare(self) -> Result<Self::Output, Self::Error> {
        Ok(match self {
            Self::Link(u) => ButtonType::Link(u),
            Self::Custom { id, style, rpc_id } => ButtonType::Custom {
                id: Ok(id?),
                style,
                rpc_id,
            },
        })
    }
}

/// Style for non-link buttons
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ButtonStyle {
    /// A primary (bold) button
    Primary,
    /// A secondary (faint) button
    Secondary,
    /// A success (green) button
    Success,
    /// A danger (red) button
    Danger,
}

impl From<ButtonStyle> for ButtonStyleModel {
    fn from(style: ButtonStyle) -> Self {
        match style {
            ButtonStyle::Primary => ButtonStyleModel::Primary,
            ButtonStyle::Secondary => ButtonStyleModel::Secondary,
            ButtonStyle::Success => ButtonStyleModel::Success,
            ButtonStyle::Danger => ButtonStyleModel::Danger,
        }
    }
}

/// A single dropdown menu component
#[derive(Debug)]
pub struct Menu<I, E> {
    id: Result<id::Id<'static>, E>,
    ty: MenuType<I, E>,
    placeholder: Option<String>,
    min_count: u8,
    max_count: Option<u8>,
    disabled: bool,
    rpc_id: PhantomData<fn(I)>,
}

impl<I, E> Prepare for Menu<I, E> {
    type Error = E;
    type Output = Menu<I, Infallible>;

    fn prepare(self) -> Result<Self::Output, Self::Error> {
        let Self {
            id,
            ty,
            placeholder,
            min_count,
            max_count,
            disabled,
            rpc_id,
        } = self;
        Ok(Menu {
            id: Ok(id?),
            ty: ty.prepare()?,
            placeholder,
            min_count,
            max_count,
            disabled,
            rpc_id,
        })
    }
}

impl<I> From<Menu<I, Infallible>> for CreateSelectMenu {
    fn from(value: Menu<I, Infallible>) -> Self {
        let Menu {
            id,
            ty,
            placeholder,
            min_count,
            max_count,
            disabled,
            rpc_id: _,
        } = value;
        // TODO: use into_ok() for id
        CreateSelectMenu::new(id.unwrap_or_else(|_| unreachable!()).to_string(), ty.into())
            .fold_opt(placeholder, CreateSelectMenu::placeholder)
            .min_values(min_count)
            .fold_opt(max_count, CreateSelectMenu::max_values)
            .disabled(disabled)
    }
}

#[derive(Debug)]
enum MenuType<I, E> {
    String {
        items: HashMap<id::Id<'static>, MenuItem>,
        order: Vec<Result<id::Id<'static>, E>>,
        default: HashSet<usize>,
        rpc_id: PhantomData<fn(I)>,
    },
    User(HashSet<UserId>),
    Role(HashSet<RoleId>),
    Mention(HashSet<UserId>, HashSet<RoleId>),
    Channel(HashSet<ChannelType>, HashSet<ChannelId>),
}

impl<I, E> Prepare for MenuType<I, E> {
    type Error = E;
    type Output = MenuType<I, Infallible>;

    fn prepare(self) -> Result<Self::Output, Self::Error> {
        Ok(match self {
            Self::String {
                items,
                order,
                default,
                rpc_id,
            } => MenuType::String {
                items,
                order: order
                    .into_iter()
                    .map(|r| r.map(Ok))
                    .collect::<Result<Vec<_>, _>>()?,
                default,
                rpc_id,
            },
            Self::User(u) => MenuType::User(u),
            Self::Role(r) => MenuType::Role(r),
            Self::Mention(u, r) => MenuType::Mention(u, r),
            Self::Channel(c, u) => MenuType::Channel(c, u),
        })
    }
}

impl<I> From<MenuType<I, Infallible>> for CreateSelectMenuKind {
    fn from(value: MenuType<I, Infallible>) -> Self {
        fn opt_vec<T>(set: HashSet<T>) -> Option<Vec<T>> {
            if set.is_empty() {
                None
            } else {
                Some(set.into_iter().collect())
            }
        }

        match value {
            MenuType::String {
                mut items,
                order,
                default,
                rpc_id: _,
            } => {
                let ret = Self::String {
                    options: order
                        .into_iter()
                        .enumerate()
                        .map(|(i, id)| {
                            // TODO: use into_ok
                            let id = id.unwrap_or_else(|_| unreachable!());

                            items
                                .remove(&id)
                                .unwrap_or_else(|| unreachable!())
                                .build(&id)
                                .default_selection(default.contains(&i))
                        })
                        .collect(),
                };

                if !items.is_empty() {
                    unreachable!("Trailing items in MenuType::String");
                }

                ret
            },
            MenuType::User(u) => Self::User {
                default_users: opt_vec(u),
            },
            MenuType::Role(r) => Self::Role {
                default_roles: opt_vec(r),
            },
            MenuType::Mention(u, r) => Self::Mentionable {
                default_users: opt_vec(u),
                default_roles: opt_vec(r),
            },
            MenuType::Channel(c, d) => Self::Channel {
                channel_types: opt_vec(c),
                default_channels: opt_vec(d),
            },
        }
    }
}

/// An item from a dropdown menu
#[derive(Debug)]
pub struct MenuItem {
    label: String,
    desc: Option<String>,
    emoji: Option<ReactionType>,
}

impl MenuItem {
    fn build(self, id: &id::Id<'_>) -> CreateSelectMenuOption {
        let Self { label, desc, emoji } = self;
        CreateSelectMenuOption::new(label, id.to_string())
            .fold_opt(desc, CreateSelectMenuOption::description)
            .fold_opt(emoji, CreateSelectMenuOption::emoji)
    }
}

impl From<String> for MenuItem {
    fn from(label: String) -> Self {
        Self {
            label,
            desc: None,
            emoji: None,
        }
    }
}

impl From<&str> for MenuItem {
    fn from(label: &str) -> Self {
        Self {
            label: label.into(),
            desc: None,
            emoji: None,
        }
    }
}

/// A textbox component, valid only for modals
#[derive(Debug)]
pub struct TextInput<I, E> {
    id: Result<id::Id<'static>, E>,
    style: InputTextStyle,
    label: String,
    min_len: Option<u16>,
    max_len: Option<u16>,
    required: bool,
    value: String,
    placeholder: Option<String>,
    rpc_id: PhantomData<fn(I)>,
}

impl<I: ComponentId> TextInput<I, id::Error> {
    #[inline]
    fn new(payload: I::Payload, style: InputTextStyle, label: impl Into<String>) -> Self {
        Self {
            id: id::write(&I::from_parts(payload)),
            style,
            label: label.into(),
            min_len: None,
            max_len: None,
            required: true,
            value: String::new(),
            placeholder: None,
            rpc_id: PhantomData,
        }
    }

    /// Construct a new short textbox
    ///
    /// # Errors
    /// This function returns an error if the given ID payload cannot be encoded
    /// correctly.
    #[inline]
    pub fn short(payload: I::Payload, label: impl Into<String>) -> Self {
        Self::new(payload, InputTextStyle::Short, label)
    }

    /// Construct a new paragraph textbox
    ///
    /// # Errors
    /// This function returns an error if the given ID payload cannot be encoded
    /// correctly.
    #[inline]
    pub fn long(payload: I::Payload, label: impl Into<String>) -> Self {
        Self::new(payload, InputTextStyle::Paragraph, label)
    }
}

#[builder(trait_name = TextInputExt)]
/// Helper methods for mutating [`TextInput`]
impl<I, E> TextInput<I, E> {
    /// Set the valid length range for this textbox
    pub fn len(&mut self, len: impl BuildRange<u16>) {
        let (min_len, max_len) = len.build_range().into_inner();
        self.min_len = min_len;
        self.max_len = max_len;
    }

    /// Set whether this textbox is a required field
    pub fn required(&mut self, req: bool) { self.required = req; }

    /// Set the pre-filled value for this textbox
    pub fn value(&mut self, val: impl Into<String>) { self.value = val.into(); }

    /// Set the empty placeholder text for this textbox
    pub fn placeholder(&mut self, placeholder: impl Into<String>) {
        self.placeholder = Some(placeholder.into());
    }
}

impl<I, E> Prepare for TextInput<I, E> {
    type Error = E;
    type Output = TextInput<I, Infallible>;

    fn prepare(self) -> Result<Self::Output, Self::Error> {
        let Self {
            id,
            style,
            label,
            min_len,
            max_len,
            required,
            value,
            placeholder,
            rpc_id,
        } = self;
        Ok(TextInput {
            id: Ok(id?),
            style,
            label,
            min_len,
            max_len,
            required,
            value,
            placeholder,
            rpc_id,
        })
    }
}

impl<I> From<TextInput<I, Infallible>> for CreateInputText {
    fn from(value: TextInput<I, Infallible>) -> Self {
        let TextInput {
            id,
            style,
            label,
            min_len,
            max_len,
            required,
            value,
            placeholder,
            rpc_id: _,
        } = value;
        // TODO: use into_ok() for id
        Self::new(
            style,
            label,
            id.unwrap_or_else(|_| unreachable!()).to_string(),
        )
        .fold_opt(min_len, Self::min_length)
        .fold_opt(max_len, Self::max_length)
        .required(required)
        .value(value)
        .fold_opt(placeholder, Self::placeholder)
    }
}

impl<I> From<TextInput<I, Infallible>> for CreateActionRow {
    #[inline]
    fn from(value: TextInput<I, Infallible>) -> Self { Self::InputText(value.into()) }
}
