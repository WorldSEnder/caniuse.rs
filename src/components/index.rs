use std::time::Duration;

use stdweb::{js, unstable::TryInto, web::window};
use yew::{
    html,
    services::timeout::{TimeoutService, TimeoutTask},
    Component, ComponentLink, Html, Properties, ShouldRender,
};

use crate::{
    components::FeatureEntry,
    search::extract_search_terms,
    services::{
        resize::{ResizeService, ResizeTask},
        scroll::{ScrollService, ScrollTask},
    },
    FeatureData, FEATURES,
};

pub struct Index {
    link: ComponentLink<Self>,
    current_search_terms: Vec<String>,
    current_search_results: Vec<FeatureData>,
    items_visible: usize,

    _scroll_task: ScrollTask,
    _resize_task: ResizeTask,
    _timeout_task: TimeoutTask,
}

pub enum Msg {
    Update,
}

#[derive(Clone, Properties)]
pub struct Props {
    #[props(required)]
    pub search_query: String,
}

const BATCH_SIZE: usize = 12;

impl Component for Index {
    type Message = Msg;
    type Properties = Props;

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        let _scroll_task = ScrollService::new().register(link.callback(|_| Msg::Update));
        let _resize_task = ResizeService::new().register(link.callback(|_| Msg::Update));
        let _timeout_task =
            TimeoutService::new().spawn(Duration::from_secs(0), link.callback(|_| Msg::Update));

        Self {
            link,
            current_search_terms: Vec::new(),
            current_search_results: Vec::new(),
            items_visible: BATCH_SIZE,

            _scroll_task,
            _resize_task,
            _timeout_task,
        }
    }

    fn update(&mut self, msg: Msg) -> ShouldRender {
        match msg {
            Msg::Update => {
                let distance_to_bottom: f64 =
                    js! { return document.body.scrollHeight - window.scrollY - window.innerHeight; }
                    .try_into()
                    .unwrap();

                if distance_to_bottom < window().inner_height() as f64 {
                    self.items_visible += BATCH_SIZE;
                    self._timeout_task = TimeoutService::new()
                        .spawn(Duration::from_secs(0), self.link.callback(|_| Msg::Update));

                    true
                } else {
                    false
                }
            }
        }
    }

    fn change(&mut self, props: Props) -> ShouldRender {
        let search_terms = extract_search_terms(&props.search_query).unwrap_or_default();
        let features_to_search = if !self.current_search_terms.is_empty()
            && self.current_search_terms.len() <= search_terms.len()
        {
            let len = self.current_search_terms.len();

            if self.current_search_terms[..] == search_terms[..len]
                || (self.current_search_terms[..len - 1] == search_terms[..len - 1]
                    && search_terms[len - 1].starts_with(&self.current_search_terms[len - 1]))
            {
                &self.current_search_results
            } else {
                FEATURES
            }
        } else {
            FEATURES
        };

        self.current_search_results = if search_terms.is_empty() {
            Vec::new()
        } else {
            features_to_search.iter().filter(|f| f.does_match(&search_terms)).copied().collect()
        };
        self.current_search_terms = search_terms;

        self.items_visible = BATCH_SIZE;
        self._timeout_task = TimeoutService::new()
            .spawn(Duration::from_secs(0), self.link.callback(|_| Msg::Update));

        true
    }

    fn view(&self) -> Html {
        let features = if self.current_search_terms.is_empty() {
            let list = FEATURES.iter().map(|&f| html! { <FeatureEntry data=f /> });
            html! { { for list.take(self.items_visible) } }
        } else {
            let list = self.current_search_results.iter().map(|&f| {
                let m = f.get_matches(&self.current_search_terms);
                html! { <FeatureEntry data=f match_=m /> }
            });

            html! { { for list.take(self.items_visible) } }
        };

        html! {
            <ul class="feature-list">{ features }</ul>
        }
    }
}
