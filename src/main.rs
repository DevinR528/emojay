use std::{
    convert::TryInto,
    ptr,
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

use druid::{
    lens, theme,
    widget::{
        Container, Flex, Label, List, ListIter, MainAxisAlignment, Painter, Scroll,
        TextBox,
    },
    AppDelegate, AppLauncher, Application, Color, Command, Data, DelegateCtx, Env,
    EventCtx, Handled, Lens, LocalizedString, RenderContext, Selector, Target, UnitPoint,
    Widget, WidgetExt, WindowDesc,
};
use fuzzy_matcher as fz;

mod mojis;

const COPY: Selector<Emoji> = Selector::new("emoji.copy");

static INTERN: (AtomicUsize, AtomicPtr<(&'static str, &'static str)>) =
    (AtomicUsize::new(0), AtomicPtr::new(ptr::null_mut()));

struct EmojiCopy;

impl AppDelegate<EmojiStuff> for EmojiCopy {
    fn command(
        &mut self,
        _ctx: &mut DelegateCtx,
        _target: Target,
        cmd: &Command,
        _data: &mut EmojiStuff,
        _env: &Env,
    ) -> Handled {
        if let Some(emoji) = cmd.get(COPY) {
            Application::global().clipboard().put_string(emoji.0.1);
            Handled::Yes
        } else {
            Handled::No
        }
    }
}

/// The text description and the emoji.
#[derive(Debug, Clone, Copy, Data)]
#[repr(transparent)]
struct Emoji((&'static str, &'static str));

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
struct EmojiList(&'static [Emoji]);

impl EmojiList {
    pub fn new(emoji: &'static [(&'static str, &'static str)]) -> Self {
        unsafe { std::mem::transmute(emoji) }
    }

    fn filter(&self, search: &str) -> Self {
        use fz::FuzzyMatcher;
        let matcher = fz::clangd::ClangdMatcher::default();

        let list: &'static [Emoji] = mojis::EMOJIS
            .iter()
            .copied()
            .filter(|e| {
                e.0.contains(search)
                    || search.is_empty()
                    || matcher
                        .fuzzy_match(e.0, search)
                        .map(|score| score > 25)
                        .unwrap_or(false)
            })
            .map(Emoji)
            .collect::<Vec<_>>()
            .leak();

        let len = list.len();
        let ptr = list.as_ptr() as *mut (&'static str, &'static str);

        let (l, p) = &INTERN;
        let pointer = p.load(Ordering::SeqCst);
        if !pointer.is_null() {
            let length = l.load(Ordering::SeqCst);
            let pointer = p.load(Ordering::SeqCst);
            unsafe { ptr::drop_in_place(ptr::slice_from_raw_parts_mut(pointer, length)) };
        }
        l.store(len, Ordering::SeqCst);
        p.store(ptr, Ordering::SeqCst);

        EmojiList(list)
    }
}

impl Data for EmojiList {
    fn same(&self, other: &Self) -> bool { self.data_len() == other.data_len() }
}

#[derive(Clone, Debug, Data, Lens)]
struct EmojiStuff {
    search: String,
    emojis: EmojiList,
}

#[derive(Clone, Debug, Data, Lens)]
struct EmojiState {
    stuff: EmojiStuff,
}

impl ListIter<[Emoji; 5]> for EmojiList {
    fn for_each(&self, mut cb: impl FnMut(&[Emoji; 5], usize)) {
        for (i, e) in self.0.chunks(5).enumerate() {
            let mut e = e.to_vec();
            for _ in e.len()..5 {
                e.push(Emoji((" ", "0")))
            }
            let e: [Emoji; 5] =
                e.try_into().expect("there are 1570 emojis evenly divisible by 5");
            cb(&e, i)
        }
    }

    fn for_each_mut(&mut self, mut cb: impl FnMut(&mut [Emoji; 5], usize)) {
        for (i, e) in self.0.chunks(5).enumerate() {
            let mut e = e.to_vec();
            for _ in e.len()..5 {
                e.push(Emoji((" ", "0")))
            }
            let mut e: [Emoji; 5] =
                e.try_into().expect("there are 1570 emojis evenly divisible by 5");
            cb(&mut e, i)
        }
    }

    fn data_len(&self) -> usize { self.0.len() }
}

struct EmojiPane {
    list: Flex<EmojiStuff>,
}
impl Widget<EmojiStuff> for EmojiPane {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        event: &druid::Event,
        data: &mut EmojiStuff,
        env: &Env,
    ) {
        data.emojis = data.emojis.filter(&data.search);
        self.list.event(ctx, event, data, env);
        ctx.request_paint();
    }

    fn lifecycle(
        &mut self,
        ctx: &mut druid::LifeCycleCtx,
        event: &druid::LifeCycle,
        data: &EmojiStuff,
        env: &Env,
    ) {
        self.list.lifecycle(ctx, event, data, env);
    }

    fn update(
        &mut self,
        ctx: &mut druid::UpdateCtx,
        old_data: &EmojiStuff,
        data: &EmojiStuff,
        env: &Env,
    ) {
        self.list.update(ctx, &old_data, data, env)
    }

    fn layout(
        &mut self,
        ctx: &mut druid::LayoutCtx,
        bc: &druid::BoxConstraints,
        data: &EmojiStuff,
        env: &Env,
    ) -> druid::Size {
        self.list.layout(ctx, bc, data, env)
    }

    fn paint(&mut self, ctx: &mut druid::PaintCtx, data: &EmojiStuff, env: &Env) {
        self.list.paint(ctx, &data, env);
    }
}

fn emoji_tile(idx: usize) -> Container<[Emoji; 5]> {
    let painter = Painter::new(|ctx, _, env| {
        let bounds = ctx.size().to_rect();

        ctx.fill(bounds, &env.get(theme::BACKGROUND_DARK));

        if ctx.is_hot() {
            ctx.stroke(bounds.inset(-0.5), &Color::WHITE, 1.0);
        }

        if ctx.is_active() {
            ctx.fill(bounds, &env.get(theme::PRIMARY_LIGHT));
        }
    });

    Label::new(move |emojis: &[Emoji; 5], _env: &Env| emojis[idx].0.1.to_owned())
        .with_text_size(30.0)
        .center()
        .align_vertical(UnitPoint::LEFT)
        .padding(10.0)
        .expand()
        .height(46.0)
        .background(painter)
}

fn emoji_row() -> Flex<[Emoji; 5]> {
    fn on_click(moji: &Emoji, ctx: &mut EventCtx) {
        ctx.submit_command(COPY.with(*moji));
        ctx.request_paint()
    }
    Flex::row()
        .with_spacer(1.0)
        .with_flex_child(
            emoji_tile(0).on_click(move |ctx, data: &mut [Emoji; 5], _env| {
                on_click(&data[0], ctx)
            }),
            1.0,
        )
        .with_spacer(1.0)
        .with_flex_child(
            emoji_tile(1).on_click(move |ctx, data: &mut [Emoji; 5], _env| {
                on_click(&data[1], ctx)
            }),
            1.0,
        )
        .with_spacer(1.0)
        .with_flex_child(
            emoji_tile(2).on_click(move |ctx, data: &mut [Emoji; 5], _env| {
                on_click(&data[2], ctx)
            }),
            1.0,
        )
        .with_spacer(1.0)
        .with_flex_child(
            emoji_tile(3).on_click(move |ctx, data: &mut [Emoji; 5], _env| {
                on_click(&data[3], ctx)
            }),
            1.0,
        )
        .with_spacer(1.0)
        .with_flex_child(
            emoji_tile(4).on_click(move |ctx, data: &mut [Emoji; 5], _env| {
                on_click(&data[4], ctx)
            }),
            1.0,
        )
}

fn ui_builder() -> EmojiPane {
    // `TextBox` is of type `Widget<String>`
    // via `.lens` we get it to be of type `Widget<MyComplexState>`
    let searchbar = TextBox::new()
        .with_placeholder("Search emoji's")
        .lens(lens::Map::new(
            |e: &EmojiStuff| e.search.clone(),
            |a: &mut EmojiStuff, b: String| a.search = b,
        ))
        .expand_width();
    EmojiPane {
        list: Flex::column()
            .main_axis_alignment(MainAxisAlignment::Start)
            .with_flex_spacer(0.1)
            .with_flex_child(
                Flex::row().with_flex_child(searchbar, 1.0).with_spacer(0.1),
                1.0,
            )
            .with_flex_spacer(0.1)
            .main_axis_alignment(MainAxisAlignment::Start)
            .with_flex_child(
                Scroll::new(List::new(emoji_row).with_spacing(0.4))
                    .content_must_fill(true)
                    .vertical()
                    .lens(EmojiStuff::emojis),
                8.0,
            ),
    }
}

fn main() {
    let main_window = WindowDesc::new(ui_builder())
        .window_size((298.0, 324.0))
        .title(LocalizedString::new("emoji-picker").with_placeholder("Emoji Picker"));
    let data = EmojiStuff { search: "".into(), emojis: EmojiList::new(mojis::EMOJIS) };

    AppLauncher::with_window(main_window)
        .delegate(EmojiCopy)
        .launch(data)
        .expect("launch failed");
}
