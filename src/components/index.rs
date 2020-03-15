use std::time::Duration;

use yew::{
    html,
    services::{
        resize::{ResizeService, ResizeTask},
        timeout::{TimeoutService, TimeoutTask},
    },
    Component, ComponentLink, Html, Properties, ShouldRender,
};

use crate::{
    components::FeatureEntry,
    data2::{FeatureData, FeatureToml, VersionData},
    search::{extract_search_terms, run_search},
    services::scroll::{ScrollService, ScrollTask},
    util::{document_body, window},
};

pub struct Index {
    link: ComponentLink<Self>,
    data: FeatureToml,
    current_search_terms: Vec<String>,
    current_search_results: Vec<(Option<VersionData>, FeatureData)>,
    items_visible: usize,
    search_scores: Vec<(u16, f64)>,

    _scroll_task: ScrollTask,
    _resize_task: ResizeTask,
    _timeout_task: TimeoutTask,
}

pub enum Msg {
    Update,
}

#[derive(Clone, Properties)]
pub struct Props {
    pub data: FeatureToml,
    pub search_query: String,
}

const BATCH_SIZE: usize = 12;

impl Component for Index {
    type Message = Msg;
    type Properties = Props;

    fn create(props: Props, link: ComponentLink<Self>) -> Self {
        let _scroll_task = ScrollService::new().register(link.callback(|_| Msg::Update));
        let _resize_task = ResizeService::new().register(link.callback(|_| Msg::Update));
        let _timeout_task =
            TimeoutService::new().spawn(Duration::from_secs(0), link.callback(|_| Msg::Update));

        let search_terms = extract_search_terms(&props.search_query).unwrap_or_default();
        let mut search_scores = vec![(0, 0.0); props.data.features().count()];

        let current_search_results = run_search(&props.data, &search_terms, &mut search_scores);
        let current_search_terms = search_terms;

        Self {
            link,
            data: props.data,
            current_search_terms,
            current_search_results,
            items_visible: BATCH_SIZE,
            search_scores,

            _scroll_task,
            _resize_task,
            _timeout_task,
        }
    }

    fn update(&mut self, msg: Msg) -> ShouldRender {
        match msg {
            Msg::Update => {
                let inner_height = window().inner_height().unwrap().as_f64().unwrap();
                let scroll_y = window().scroll_y().unwrap();
                let distance_to_bottom =
                    document_body().scroll_height() as f64 - scroll_y - inner_height;

                if distance_to_bottom < inner_height {
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

        self.current_search_results =
            run_search(&props.data, &search_terms, &mut self.search_scores);
        self.current_search_terms = search_terms;

        self.items_visible = BATCH_SIZE;
        self._timeout_task = TimeoutService::new()
            .spawn(Duration::from_secs(0), self.link.callback(|_| Msg::Update));

        true
    }

    fn view(&self) -> Html {
        if self.current_search_terms.is_empty() {
            let list = self.data.features().map(|(v, f)| {
                html! {
                    <FeatureEntry feature=f.clone() version=v.clone() />
                }
            });
            html! { <div class="feature-list">{ for list.take(self.items_visible) }</div> }
        } else if self.current_search_results.is_empty() {
            html! { <div class="box muted">{"Nothing found, sorry."}</div> }
        } else {
            let list = self.current_search_results.iter().map(|(v, f)| {
                html! { <FeatureEntry feature=f.clone() version=v.clone() /> }
            });

            html! { <div class="feature-list">{ for list.take(self.items_visible) }</div> }
        }
    }
}
