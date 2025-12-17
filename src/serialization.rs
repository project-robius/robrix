//! Utility type wrappers / newtypes related to serialization.

use makepad_widgets::{DockItem, LiveId, splitter::{SplitterAlign, SplitterAxis}};
use serde::{Deserialize, Serialize};

/// A version of Makepad's [`SplitterAxis`] that implements serde traits.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SplitterAxisSerde {
    Horizontal,
    Vertical
}
impl From<SplitterAxis> for SplitterAxisSerde {
    fn from(axis: SplitterAxis) -> Self {
        match axis {
            SplitterAxis::Horizontal => SplitterAxisSerde::Horizontal,
            SplitterAxis::Vertical => SplitterAxisSerde::Vertical,
        }
    }
}
impl From<SplitterAxisSerde> for SplitterAxis {
    fn from(axis: SplitterAxisSerde) -> Self {
        match axis {
            SplitterAxisSerde::Horizontal => SplitterAxis::Horizontal,
            SplitterAxisSerde::Vertical => SplitterAxis::Vertical,
        }
    }
}

/// A version of Makepad's [`SplitterAlign`] that implements serde traits.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SplitterAlignSerde {
    FromA(f64),
    FromB(f64),
    Weighted(f64),
}
impl From<makepad_widgets::splitter::SplitterAlign> for SplitterAlignSerde {
    fn from(align: SplitterAlign) -> Self {
        match align {
            SplitterAlign::FromA(v) => SplitterAlignSerde::FromA(v),
            SplitterAlign::FromB(v) => SplitterAlignSerde::FromB(v),
            SplitterAlign::Weighted(v) => SplitterAlignSerde::Weighted(v),
        }
    }
}
impl From<SplitterAlignSerde> for makepad_widgets::splitter::SplitterAlign {
    fn from(align: SplitterAlignSerde) -> Self {
        match align {
            SplitterAlignSerde::FromA(v) => SplitterAlign::FromA(v),
            SplitterAlignSerde::FromB(v) => SplitterAlign::FromB(v),
            SplitterAlignSerde::Weighted(v) => SplitterAlign::Weighted(v),
        }
    }
}

/// A version of Makepad's [`LiveId`] that implements serde traits.
#[derive(Clone, Debug, Default, Eq, Hash, Copy, Ord, PartialOrd, PartialEq, Serialize, Deserialize)]
pub struct LiveIdSerde(pub u64);
impl From<LiveId> for LiveIdSerde {
    fn from(id: LiveId) -> Self {
        LiveIdSerde(id.0)
    }
}
impl From<LiveIdSerde> for LiveId {
    fn from(id: LiveIdSerde) -> Self {
        LiveId(id.0)
    }
}

/// A version of Makepad's [`DockItem`] that implements serde traits.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DockItemSerde {
    Splitter {
        axis: SplitterAxisSerde,
        align: SplitterAlignSerde,
        a: LiveIdSerde,
        b: LiveIdSerde,
    },
    Tabs {
        tabs: Vec<LiveIdSerde>,
        selected: usize,
        closable: bool,
    },
    Tab {
        name: String,
        template: LiveIdSerde,
        kind: LiveIdSerde,
    }
}
impl From<DockItem> for DockItemSerde {
    fn from(item: DockItem) -> Self {
        match item {
            DockItem::Splitter { axis, align, a, b } => {
                DockItemSerde::Splitter {
                    axis: axis.into(),
                    align: align.into(),
                    a: a.into(),
                    b: b.into(),
                }
            }
            DockItem::Tabs { tabs, selected, closable } => {
                DockItemSerde::Tabs {
                    tabs: tabs.into_iter().map(|id| id.into()).collect(),
                    selected,
                    closable,
                }
            }
            DockItem::Tab { name, template, kind } => {
                DockItemSerde::Tab {
                    name,
                    template: template.into(),
                    kind: kind.into(),
                }
            }
        }
    }
}
impl From<DockItemSerde> for DockItem {
    fn from(item: DockItemSerde) -> Self {
        match item {
            DockItemSerde::Splitter { axis, align, a, b } => {
                DockItem::Splitter {
                    axis: axis.into(),
                    align: align.into(),
                    a: a.into(),
                    b: b.into(),
                }
            }
            DockItemSerde::Tabs { tabs, selected, closable } => {
                DockItem::Tabs {
                    tabs: tabs.into_iter().map(|id| id.into()).collect(),
                    selected,
                    closable,
                }
            }
            DockItemSerde::Tab { name, template, kind } => {
                DockItem::Tab {
                    name,
                    template: template.into(),
                    kind: kind.into(),
                }
            }
        }
    }
}
