use super::FeedNoteParams;
use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::relay::Relay;
use crate::ui::{you, GossipUi, HighlightType, Page, Theme};
use eframe::egui;
use eframe::epaint::text::LayoutJob;
use egui::{Align, Context, Key, Layout, Modifiers, RichText, ScrollArea, Ui, Vec2};
use memoize::memoize;
use nostr_types::{ContentSegment, NostrBech32, NostrUrl, ShatteredContent, Tag};

#[memoize]
pub fn textarea_highlighter(theme: Theme, text: String) -> LayoutJob {
    let mut job = LayoutJob::default();

    // Shatter
    let shattered_content = ShatteredContent::new(text.clone());

    for segment in shattered_content.segments.iter() {
        match segment {
            ContentSegment::NostrUrl(nostr_url) => {
                let chunk = format!("{}", nostr_url);
                let highlight = match nostr_url.0 {
                    NostrBech32::EventAddr(_) => HighlightType::Event,
                    NostrBech32::EventPointer(_) => HighlightType::Event,
                    NostrBech32::Id(_) => HighlightType::Event,
                    NostrBech32::Profile(_) => HighlightType::PublicKey,
                    NostrBech32::Pubkey(_) => HighlightType::PublicKey,
                    NostrBech32::Relay(_) => HighlightType::Relay,
                };
                job.append(&chunk, 0.0, theme.highlight_text_format(highlight));
            }
            ContentSegment::TagReference(i) => {
                let chunk = format!("#[{}]", i);
                // This has been unrecommended, and we have to check if 'i' is in bounds.
                // So we don't do this anymore
                // job.append(&chunk, 0.0, theme.highlight_text_format(HighlightType::Event));
                job.append(
                    &chunk,
                    0.0,
                    theme.highlight_text_format(HighlightType::Nothing),
                );
            }
            ContentSegment::Hyperlink(span) => {
                let chunk = shattered_content.slice(span).unwrap();
                job.append(
                    chunk,
                    0.0,
                    theme.highlight_text_format(HighlightType::Hyperlink),
                );
            }
            ContentSegment::Plain(span) => {
                let chunk = shattered_content.slice(span).unwrap();
                job.append(
                    chunk,
                    0.0,
                    theme.highlight_text_format(HighlightType::Nothing),
                );
            }
        }
    }

    job
}

pub(in crate::ui) fn posting_area(
    app: &mut GossipUi,
    ctx: &Context,
    frame: &mut eframe::Frame,
    ui: &mut Ui,
) {
    // Posting Area
    ui.vertical(|ui| {
        if !GLOBALS.signer.is_ready() {
            ui.horizontal_wrapped(|ui| {
                if GLOBALS.signer.encrypted_private_key().is_some() {
                    you::offer_unlock_priv_key(app, ui);
                } else {
                    ui.label("You need to ");
                    if ui.link("setup your identity").clicked() {
                        app.set_page(Page::YourKeys);
                    }
                    ui.label(" to post.");
                }
            });
        } else if GLOBALS
            .storage
            .filter_relays(|r| r.has_usage_bits(Relay::WRITE))
            .unwrap_or(vec![])
            .is_empty()
        {
            ui.horizontal_wrapped(|ui| {
                ui.label("You need to ");
                if ui.link("choose write relays").clicked() {
                    app.set_page(Page::RelaysAll);
                }
                ui.label(" to post.");
            });
        } else {
            real_posting_area(app, ctx, frame, ui);
        }
    });
}

fn real_posting_area(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    // Maybe render post we are replying to or reposting

    if let Some(id) = app.replying_to.or(app.draft_repost) {
        ScrollArea::vertical()
            .max_height(200.0)
            .override_scroll_delta(Vec2 {
                x: 0.0,
                y: app.current_scroll_offset,
            })
            .show(ui, |ui| {
                super::note::render_note(
                    app,
                    ctx,
                    frame,
                    ui,
                    FeedNoteParams {
                        id,
                        indent: 0,
                        as_reply_to: true,
                        threaded: false,
                        is_first: true,
                        is_last: true,
                    },
                );
            });
    }

    let mut send_now: bool = false;

    if app.draft_repost.is_none() {
        // Text area
        let theme = app.settings.theme;
        let mut layouter = |ui: &Ui, text: &str, wrap_width: f32| {
            let mut layout_job = textarea_highlighter(theme, text.to_owned());
            layout_job.wrap.max_width = wrap_width;
            ui.fonts(|f| f.layout_job(layout_job))
        };

        if app.include_subject && app.replying_to.is_none() {
            ui.horizontal(|ui| {
                ui.label("Subject: ");
                ui.add(
                    text_edit_line!(app, app.subject)
                        .hint_text("Type subject here")
                        .desired_width(f32::INFINITY),
                );
            });
        }

        if app.include_content_warning {
            ui.horizontal(|ui| {
                ui.label("Content Warning: ");
                ui.add(
                    text_edit_line!(app, app.content_warning)
                        .hint_text("Type content warning here")
                        .desired_width(f32::INFINITY),
                );
            });
        }

        let draft_response = ui.add(
            text_edit_multiline!(app, app.draft)
                .id_source("compose_area")
                .hint_text("Type your message here")
                .desired_width(f32::INFINITY)
                .lock_focus(true)
                .interactive(app.draft_repost.is_none())
                .layouter(&mut layouter),
        );
        if app.draft_needs_focus {
            draft_response.request_focus();
            app.draft_needs_focus = false;
        }

        if draft_response.has_focus() && !app.draft.is_empty() {
            let modifiers = if cfg!(target_os = "macos") {
                Modifiers {
                    command: true,
                    ..Default::default()
                }
            } else {
                Modifiers {
                    ctrl: true,
                    ..Default::default()
                }
            };

            if ui.input_mut(|i| i.consume_key(modifiers, Key::Enter)) {
                send_now = true;
            }
        }

        ui.add_space(8.0);
    }

    ui.horizontal(|ui| {
        if ui.button("Cancel").clicked() {
            app.clear_post();
        }

        ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
            ui.add_space(12.0);
            let send_label = if app.draft_repost.is_some() {
                "Repost"
            } else {
                "Send"
            };

            if ui.button(send_label).clicked()
                && (!app.draft.is_empty() || app.draft_repost.is_some())
            {
                send_now = true;
            }

            if app.draft_repost.is_none() {
                ui.add(
                    text_edit_line!(app, app.tag_someone)
                        .desired_width(100.0)
                        .hint_text("@username"),
                );

                if !app.tag_someone.is_empty() {
                    let pairs = GLOBALS
                        .people
                        .search_people_to_tag(&app.tag_someone)
                        .unwrap_or(vec![]);
                    if !pairs.is_empty() {
                        ui.menu_button("@", |ui| {
                            for pair in pairs {
                                if ui.button(pair.0).clicked() {
                                    if !app.draft.ends_with(' ') && !app.draft.is_empty() {
                                        app.draft.push(' ');
                                    }
                                    let nostr_url: NostrUrl = pair.1.into();
                                    app.draft.push_str(&format!("{}", nostr_url));
                                    app.tag_someone = "".to_owned();
                                }
                            }
                        });
                    }
                }

                if app.include_subject {
                    if ui.button("Remove Subject").clicked() {
                        app.include_subject = false;
                        app.subject = "".to_owned();
                    }
                } else if app.replying_to.is_none() && ui.button("Add Subject").clicked() {
                    app.include_subject = true;
                }

                if app.include_content_warning {
                    if ui.button("Remove Content Warning").clicked() {
                        app.include_content_warning = false;
                        app.content_warning = "".to_owned();
                    }
                } else if ui.button("Add Content Warning").clicked() {
                    app.include_content_warning = true;
                }

                // Emoji picker
                ui.menu_button(RichText::new("😀▼").size(14.0), |ui| {
                    if let Some(emoji) = crate::ui::components::emoji_picker(ui) {
                        app.draft.push(emoji);
                    }
                });
            }
        });
    });

    if send_now {
        let mut tags: Vec<Tag> = Vec::new();
        if app.include_content_warning {
            tags.push(Tag::ContentWarning {
                warning: app.content_warning.clone(),
                trailing: Vec::new(),
            });
        }
        if let Some(delegatee_tag) = GLOBALS.delegation.get_delegatee_tag() {
            tags.push(delegatee_tag);
        }
        if app.include_subject {
            tags.push(Tag::Subject {
                subject: app.subject.clone(),
                trailing: Vec::new(),
            });
        }
        match app.replying_to {
            Some(replying_to_id) => {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::Post(
                    app.draft.clone(),
                    tags,
                    Some(replying_to_id),
                ));
            }
            None => {
                if let Some(event_id) = app.draft_repost {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::Repost(event_id));
                } else {
                    let _ = GLOBALS.to_overlord.send(ToOverlordMessage::Post(
                        app.draft.clone(),
                        tags,
                        None,
                    ));
                }
            }
        }
        app.clear_post();
    }

    // List tags that will be applied
    // FIXME: list tags from parent event too in case of reply
    // FIXME: tag handling in overlord::post() needs to move back here so the user can control this
    for (i, bech32) in NostrBech32::find_all_in_string(&app.draft)
        .iter()
        .enumerate()
    {
        let pk = match bech32 {
            NostrBech32::Pubkey(pk) => pk,
            NostrBech32::Profile(prof) => &prof.pubkey,
            _ => continue,
        };
        let rendered = if let Ok(Some(person)) = GLOBALS.storage.read_person(pk) {
            match person.name() {
                Some(name) => name.to_owned(),
                None => format!("{}", bech32),
            }
        } else {
            format!("{}", bech32)
        };

        ui.label(format!("{}: {}", i, rendered));
    }
}
