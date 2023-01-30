use qcore::{build_range::BuildRange, builder};
use serenity::{
    builder::{
        CreateActionRow, CreateButton, CreateInputText, CreateInteractionResponseData,
        CreateInteractionResponseFollowup, CreateSelectMenu, CreateSelectMenuOption,
        EditInteractionResponse,
    },
    model::{
        application::component::{ButtonStyle as ButtonStyleModel, InputTextStyle},
        channel::{ChannelType, ReactionType},
    },
};

use super::{super::rpc::ComponentId, id, ResponseData};
use crate::prelude::*;

mod private {
    use serenity::builder::CreateActionRow;

    pub trait BuildComponent {
        fn build_component(self, row: &mut CreateActionRow) -> &mut CreateActionRow;
    }
}

#[derive(Debug)]
pub struct Components<I, T, E>(pub(super) Vec<ActionRow<I, T, E>>);

impl<I, T, E> Default for Components<I, T, E> {
    fn default() -> Self { Self(vec![]) }
}

macro_rules! build_components {
    ($self:expr, $builder:expr) => {{
        let Self(rows) = $self;
        $builder.components(|b| {
            rows.into_iter().fold(b, |b, r| {
                let ActionRow {
                    err,
                    components,
                    id: _,
                } = r;
                assert!(err.0.is_none());
                b.create_action_row(|b| components.into_iter().fold(b, |b, c| c.build_component(b)))
            })
        })
    }};
}

impl<I, T, E> Components<I, T, E> {
    #[inline]
    pub fn prepare(self) -> Result<Components<I, T, Infallible>, E> {
        let Self(rows) = self;
        Ok(Components(
            rows.into_iter()
                .map(ActionRow::prepare)
                .collect::<Result<_, E>>()?,
        ))
    }
}

impl<I, T: private::BuildComponent> Components<I, T, Infallible> {
    #[inline]
    pub(super) fn build_edit_response(
        self,
        res: &mut EditInteractionResponse,
    ) -> &mut EditInteractionResponse {
        build_components!(self, res)
    }

    #[inline]
    pub(super) fn build_followup<'a, 'b>(
        self,
        fup: &'a mut CreateInteractionResponseFollowup<'b>,
    ) -> &'a mut CreateInteractionResponseFollowup<'b> {
        build_components!(self, fup).components(|c| c)
    }
}

#[builder(trait_name = "ComponentsExt")]
impl<I, T, E> Components<I, T, E> {
    pub fn row(&mut self, row: ActionRow<I, T, E>) { self.0.push(row); }

    #[inline]
    pub fn build_row(&mut self, f: impl FnOnce(ActionRow<I, T, E>) -> ActionRow<I, T, E>) {
        self.row(f(ActionRow::default()));
    }
}

impl<'a, I, T: private::BuildComponent> ResponseData<'a> for Components<I, T, Infallible> {
    fn build_response_data<'b>(
        self,
        data: &'b mut CreateInteractionResponseData<'a>,
    ) -> &'b mut CreateInteractionResponseData<'a> {
        build_components!(self, data)
    }
}

// TODO: clean this up
#[derive(Debug)]
struct RowError<E>(Option<E>);

impl<E> RowError<E> {
    fn catch(&mut self, f: impl FnOnce() -> Result<(), E>) {
        match (&mut self.0, f()) {
            (_, Ok(())) | (Some(_), Err(_)) => (),
            (e @ None, Err(f)) => *e = Some(f),
        }
    }
}

#[derive(Debug)]
pub struct ActionRow<I, T, E> {
    err: RowError<E>,
    components: Vec<T>,
    id: PhantomData<fn(I)>,
}

impl<I, T, E> Default for ActionRow<I, T, E> {
    fn default() -> Self {
        Self {
            err: RowError(None),
            components: vec![],
            id: PhantomData::default(),
        }
    }
}

impl<I, T, E> ActionRow<I, T, E> {
    #[inline]
    fn prepare(self) -> Result<ActionRow<I, T, Infallible>, E> {
        let Self {
            err,
            components,
            id,
        } = self;
        if let RowError(Some(err)) = err {
            return Err(err);
        }

        Ok(ActionRow {
            err: RowError(None),
            components,
            id,
        })
    }
}

impl<I: ComponentId> ActionRow<I, MessageComponent, id::Error> {
    fn menu_parts(
        &mut self,
        payload: I::Payload,
        ty: Result<MenuType, id::Error>,
        placeholder: impl Into<Option<String>>,
        count: impl BuildRange<u8>,
        disabled: bool,
    ) {
        self.err.catch(|| {
            let (min_count, max_count) = count.build_range().into_inner();
            self.components.push(MessageComponent {
                ty: MessageComponentType::Menu {
                    id: id::write(&I::from_parts(payload))?,
                    ty: ty?,
                    placeholder: placeholder.into(),
                    min_count: min_count.unwrap_or(0),
                    max_count, // max allowed
                },
                disabled,
            });

            Ok(())
        });
    }
}

#[builder(trait_name = "MessageActionRow")]
impl<I: ComponentId> ActionRow<I, MessageComponent, id::Error> {
    pub fn button(
        &mut self,
        payload: I::Payload,
        style: ButtonStyle,
        label: impl Into<ButtonLabel>,
        disabled: bool,
    ) {
        self.err.catch(|| {
            self.components.push(MessageComponent {
                ty: MessageComponentType::Button {
                    label: label.into(),
                    ty: ButtonType::Custom {
                        id: id::write(&I::from_parts(payload))?,
                        style,
                    },
                },
                disabled,
            });

            Ok(())
        });
    }

    pub fn link_button(
        &mut self,
        url: impl Into<Url>,
        label: impl Into<ButtonLabel>,
        disabled: bool,
    ) {
        self.components.push(MessageComponent {
            ty: MessageComponentType::Button {
                label: label.into(),
                ty: ButtonType::Link(url.into()),
            },
            disabled,
        });
    }

    pub fn menu<J: Into<MenuItem>>(
        &mut self,
        payload: I::Payload,
        placeholder: impl Into<Option<String>>,
        count: impl BuildRange<u8>,
        disabled: bool,
        default: impl IntoIterator<Item = usize>,
        options: impl IntoIterator<Item = (I::Payload, J)>,
    ) {
        let res = options.into_iter().try_fold(
            (HashMap::new(), vec![]),
            |(mut items, mut order), (payload, item)| {
                let id = id::write(&I::from_parts(payload))?;

                order.push(id.clone());
                assert!(items.insert(id, item.into()).is_none());

                Ok((items, order))
            },
        );

        let default: HashSet<_> = default.into_iter().collect();
        if let Ok((_, ref order)) = res {
            assert!(default.iter().all(|d| order.len() > *d));
        }

        self.menu_parts(
            payload,
            res.map(|(items, order)| MenuType::String {
                items,
                order,
                default,
            }),
            placeholder,
            count,
            disabled,
        );
    }

    #[inline]
    pub fn user_menu(
        &mut self,
        payload: I::Payload,
        placeholder: impl Into<Option<String>>,
        count: impl BuildRange<u8>,
        disabled: bool,
    ) {
        self.menu_parts(payload, Ok(MenuType::User), placeholder, count, disabled);
    }

    #[inline]
    pub fn role_menu(
        &mut self,
        payload: I::Payload,
        placeholder: impl Into<Option<String>>,
        count: impl BuildRange<u8>,
        disabled: bool,
    ) {
        self.menu_parts(payload, Ok(MenuType::Role), placeholder, count, disabled);
    }

    #[inline]
    pub fn mention_menu(
        &mut self,
        payload: I::Payload,
        placeholder: impl Into<Option<String>>,
        count: impl BuildRange<u8>,
        disabled: bool,
    ) {
        self.menu_parts(payload, Ok(MenuType::Mention), placeholder, count, disabled);
    }

    #[inline]
    pub fn channel_menu(
        &mut self,
        payload: I::Payload,
        placeholder: impl Into<Option<String>>,
        count: impl BuildRange<u8>,
        disabled: bool,
        tys: impl IntoIterator<Item = ChannelType>,
    ) {
        self.menu_parts(
            payload,
            Ok(MenuType::Channel(tys.into_iter().collect())),
            placeholder,
            count,
            disabled,
        );
    }
}

#[builder(trait_name = "ModalActionRow")]
impl<I: ComponentId> ActionRow<I, TextInput<I>, id::Error> {
    #[inline]
    pub fn text(&mut self, input: TextInput<I>) { self.components.push(input); }

    #[inline]
    pub fn build_text_short(
        &mut self,
        payload: I::Payload,
        label: impl Into<String>,
        f: impl FnOnce(TextInput<I>) -> TextInput<I>,
    ) {
        self.err.catch(|| {
            self.components.push(f(TextInput::short(payload, label)?));
            Ok(())
        });
    }

    #[inline]
    pub fn build_text_long(
        &mut self,
        payload: I::Payload,
        label: impl Into<String>,
        f: impl FnOnce(TextInput<I>) -> TextInput<I>,
    ) {
        self.err.catch(|| {
            self.components.push(f(TextInput::long(payload, label)?));
            Ok(())
        });
    }
}

#[inline]
fn visit<T, V: IntoIterator>(
    vals: V,
    row: &mut T,
    f: impl FnMut(&mut T, V::Item) -> &mut T,
) -> &mut T {
    vals.into_iter().fold(row, f)
}

// TODO: add constructors and an ID type parameter
#[derive(Debug)]
pub struct MessageComponent {
    ty: MessageComponentType,
    disabled: bool,
}

impl private::BuildComponent for MessageComponent {
    fn build_component(self, row: &mut CreateActionRow) -> &mut CreateActionRow {
        let Self { ty, disabled } = self;
        match ty {
            MessageComponentType::Button { label, ty } => row.create_button(|b| {
                b.disabled(disabled);
                match label {
                    ButtonLabel::Text(e, t) => visit(e, b.label(t), CreateButton::emoji),
                    ButtonLabel::Emoji(r) => b.emoji(r),
                };
                match ty {
                    ButtonType::Link(u) => b.style(ButtonStyleModel::Link).url(u),
                    ButtonType::Custom { id, style } => b.custom_id(id).style(style.into()),
                }
            }),
            MessageComponentType::Menu {
                id,
                ty,
                placeholder,
                min_count,
                max_count,
            } => row.create_select_menu(|b| {
                b.disabled(disabled).custom_id(id);
                visit(placeholder, b, CreateSelectMenu::placeholder).min_values(min_count.into());
                visit(max_count, b, |b, c| b.max_values(c.into()));
                match ty {
                    MenuType::String {
                        mut items,
                        order,
                        default,
                    } => b.options(|b| {
                        for (i, id) in order.into_iter().enumerate() {
                            let MenuItem { label, desc, emoji } =
                                items.remove(&id).unwrap_or_else(|| unreachable!());

                            b.create_option(|b| {
                                b.value(id).label(label);
                                visit(desc, b, CreateSelectMenuOption::description);
                                visit(emoji, b, CreateSelectMenuOption::emoji);
                                b.default_selection(default.contains(&i))
                            });
                        }
                        assert!(items.is_empty());
                        b
                    }),
                    t => todo!("Select menu type {t:?} unsupported by Serenity"),
                    // MenuType::User => todo!(),
                    // MenuType::Role => todo!(),
                    // MenuType::Mention => todo!(),
                    // MenuType::Channel(tys) => todo!(),
                }
            }),
        }
    }
}

#[derive(Debug)]
enum MessageComponentType {
    Button {
        label: ButtonLabel,
        ty: ButtonType,
    },
    Menu {
        id: id::Id<'static>,
        ty: MenuType,
        placeholder: Option<String>,
        min_count: u8,
        max_count: Option<u8>,
    },
}

#[derive(Debug)]
pub enum ButtonLabel {
    Text(Option<ReactionType>, String),
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

#[derive(Debug)]
pub enum ButtonType {
    Link(Url),
    Custom {
        id: id::Id<'static>,
        style: ButtonStyle,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ButtonStyle {
    Primary,
    Secondary,
    Success,
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

#[derive(Debug)]
enum MenuType {
    String {
        items: HashMap<id::Id<'static>, MenuItem>,
        order: Vec<id::Id<'static>>,
        default: HashSet<usize>,
    },
    User,
    Role,
    Mention,
    Channel(HashSet<ChannelType>),
}

#[derive(Debug)]
pub struct MenuItem {
    label: String,
    desc: Option<String>,
    emoji: Option<ReactionType>,
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

#[derive(Debug)]
pub struct TextInput<I> {
    id: id::Id<'static>,
    style: InputTextStyle,
    label: String,
    min_len: Option<u64>,
    max_len: Option<u64>,
    required: bool,
    value: String,
    placeholder: Option<String>,
    rpc_id: PhantomData<fn(I)>,
}

impl<I: ComponentId> TextInput<I> {
    #[inline]
    fn new(
        payload: I::Payload,
        style: InputTextStyle,
        label: impl Into<String>,
    ) -> Result<Self, id::Error> {
        Ok(Self {
            id: id::write(&I::from_parts(payload))?,
            style,
            label: label.into(),
            min_len: None,
            max_len: None,
            required: true,
            value: String::new(),
            placeholder: None,
            rpc_id: PhantomData::default(),
        })
    }

    #[inline]
    pub fn short(payload: I::Payload, label: impl Into<String>) -> Result<Self, id::Error> {
        Self::new(payload, InputTextStyle::Short, label)
    }

    #[inline]
    pub fn long(payload: I::Payload, label: impl Into<String>) -> Result<Self, id::Error> {
        Self::new(payload, InputTextStyle::Paragraph, label)
    }
}

#[builder(trait_name = "TextInputExt")]
impl<I> TextInput<I> {
    pub fn len(&mut self, len: impl BuildRange<u64>) {
        let (min_len, max_len) = len.build_range().into_inner();
        self.min_len = min_len;
        self.max_len = max_len;
    }

    pub fn optional(&mut self) { self.required = false; }

    pub fn value(&mut self, val: impl Into<String>) { self.value = val.into(); }

    pub fn placeholder(&mut self, placeholder: impl Into<String>) {
        self.placeholder = Some(placeholder.into());
    }
}

impl<I> private::BuildComponent for TextInput<I> {
    fn build_component(self, row: &mut CreateActionRow) -> &mut CreateActionRow {
        let Self {
            id,
            style,
            label,
            min_len,
            max_len,
            required,
            value,
            placeholder,
            rpc_id: _,
        } = self;
        row.create_input_text(|t| {
            t.custom_id(id).style(style).label(label);
            visit(min_len, t, CreateInputText::min_length);
            visit(max_len, t, CreateInputText::max_length)
                .required(required)
                .value(value);
            visit(placeholder, t, CreateInputText::placeholder)
        })
    }
}
