use crate::app::types::{MeasureState, MeasureStep};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonVariant, Dialog, DialogSize, HStack, StackAlign, StackJustify, 
    StackSpacing, Text, TextSize, TextWeight, VStack, Select, SelectOption,
};
use d3rs::scale::{LinearScale, LogScale};
use d3rs::shape::{LineConfig, LinePoint, render_line};
use d3rs::color::D3Color;

impl PlayerView {
    pub(crate) fn render_measure_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        
        let measure_state = if let Some(ms) = &state.app.measure_state {
             ms.clone()
        } else {
            return div().into_any_element();
        };

        let step_content = match measure_state.step {
            MeasureStep::DeviceSelection => self.render_measure_device_selection(&measure_state, &theme, cx).into_any_element(),
            MeasureStep::SignalConfig => self.render_measure_signal_config(&measure_state, &theme, cx).into_any_element(),
            MeasureStep::Running => self.render_measure_running(&measure_state, &theme).into_any_element(),
            MeasureStep::Results => self.render_measure_results(&measure_state, &theme).into_any_element(),
        };

        let title = match measure_state.step {
            MeasureStep::DeviceSelection => "Measurement - Device Setup",
            MeasureStep::SignalConfig => "Measurement - Signal Configuration",
            MeasureStep::Running => "Measurement - Running...",
            MeasureStep::Results => "Measurement - Results",
        };

        Dialog::new("measure-dialog")
            .title(title)
            .size(DialogSize::Lg)
            .content(step_content)
            .footer(self.render_measure_footer(&measure_state, &theme, cx))
            .into_any_element()
    }

    fn render_measure_device_selection(
        &self,
        measure_state: &MeasureState,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let out_idx = state.app.selected_output_device_index;
        let max_out = state.app.output_devices.get(out_idx)
             .and_then(|d| d.default_config.as_ref())
             .map(|c| c.channels as usize)
             .unwrap_or(2);
             
        let in_idx = state.app.selected_input_device_index;
        let max_in = state.app.input_devices.get(in_idx)
             .and_then(|d| d.default_config.as_ref())
             .map(|c| c.channels as usize)
             .unwrap_or(2);
        
        let out_options: Vec<SelectOption> = (0..max_out).map(|i| {
             SelectOption::new(format!("{}", i), format!("Channel {}", i + 1))
        }).collect();
        
        let in_options: Vec<SelectOption> = (0..max_in).map(|i| {
             SelectOption::new(format!("{}", i), format!("Channel {}", i + 1))
        }).collect();
        
        let weak_view = cx.entity().downgrade();
        let weak_view2 = cx.entity().downgrade();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Output Device").weight(TextWeight::Semibold))
                    .child(Text::new(state.app.current_output_device_name.as_deref().unwrap_or("None").to_string()).muted(true))
                    // Select for output channel
                    .child(
                         div()
                            .on_mouse_up(MouseButton::Left, cx.listener(move |view, _, _, cx| {
                                view.state.update(cx, |state, _| {
                                     if let Some(ms) = &mut state.app.measure_state {
                                         ms.output_ch_open = !ms.output_ch_open;
                                         if ms.output_ch_open { ms.input_ch_open = false; }
                                     }
                                });
                            }))
                            .child(
                                Select::new("out_ch_sel")
                                    .options(out_options)
                                    .selected(format!("{}", measure_state.output_channel))
                                    .is_open(measure_state.output_ch_open)
                                    .on_change(move |val, _, cx| {
                                         let idx = val.to_string().parse::<usize>().unwrap_or(0);
                                         weak_view.update(cx, |view, cx| {
                                             view.state.update(cx, |state, _| {
                                                 if let Some(ms) = &mut state.app.measure_state {
                                                     ms.output_channel = idx;
                                                     ms.output_ch_open = false;
                                                 }
                                             });
                                         }).ok();
                                    })
                            )
                    )
            )
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Input Device").weight(TextWeight::Semibold))
                    .child(Text::new(state.app.current_input_device_name.as_deref().unwrap_or("None").to_string()).muted(true))
                    // Select for input channel
                    .child(
                         div()
                            .on_mouse_up(MouseButton::Left, cx.listener(move |view, _, _, cx| {
                                view.state.update(cx, |state, _| {
                                     if let Some(ms) = &mut state.app.measure_state {
                                         ms.input_ch_open = !ms.input_ch_open;
                                         if ms.input_ch_open { ms.output_ch_open = false; }
                                     }
                                });
                            }))
                            .child(
                                Select::new("in_ch_sel")
                                    .options(in_options)
                                    .selected(format!("{}", measure_state.input_channel))
                                    .is_open(measure_state.input_ch_open)
                                    .on_change(move |val, _, cx| {
                                         let idx = val.to_string().parse::<usize>().unwrap_or(0);
                                         weak_view2.update(cx, |view, cx| {
                                             view.state.update(cx, |state, _| {
                                                 if let Some(ms) = &mut state.app.measure_state {
                                                     ms.input_channel = idx;
                                                     ms.input_ch_open = false;
                                                 }
                                             });
                                         }).ok();
                                    })
                            )
                    )
            )
    }

    fn render_measure_signal_config(
        &self,
        measure_state: &MeasureState,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Signal Type").weight(TextWeight::Semibold))
                    .child(
                        HStack::new().spacing(StackSpacing::Sm)
                        .child(self.render_option_chip("Sweep", measure_state.signal_type == "sweep", cx, |state, val| {
                            if let Some(ms) = &mut state.app.measure_state { ms.signal_type = "sweep".to_string(); }
                        }))
                        .child(self.render_option_chip("Pink Noise", measure_state.signal_type == "pink-noise", cx, |state, val| {
                             if let Some(ms) = &mut state.app.measure_state { ms.signal_type = "pink-noise".to_string(); }
                        }))
                    )
            )
            .child(
                 VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Duration (s)").weight(TextWeight::Semibold))
                     .child(
                        HStack::new().spacing(StackSpacing::Sm)
                        .child(self.render_option_chip("2.0s", measure_state.duration == "2.0", cx, |state, val| {
                            if let Some(ms) = &mut state.app.measure_state { ms.duration = "2.0".to_string(); }
                        }))
                         .child(self.render_option_chip("5.0s", measure_state.duration == "5.0", cx, |state, val| {
                            if let Some(ms) = &mut state.app.measure_state { ms.duration = "5.0".to_string(); }
                        }))
                         .child(self.render_option_chip("10.0s", measure_state.duration == "10.0", cx, |state, val| {
                            if let Some(ms) = &mut state.app.measure_state { ms.duration = "10.0".to_string(); }
                        }))
                    )
            )
             .child(
                 VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Level (dB)").weight(TextWeight::Semibold))
                     .child(
                        HStack::new().spacing(StackSpacing::Sm)
                        // Simple level selection for now
                         .child(self.render_option_chip("-10 dB", (measure_state.level - (-10.0)).abs() < 0.1, cx, |state, val| {
                            if let Some(ms) = &mut state.app.measure_state { ms.level = -10.0; }
                        }))
                         .child(self.render_option_chip("-20 dB", (measure_state.level - (-20.0)).abs() < 0.1, cx, |state, val| {
                            if let Some(ms) = &mut state.app.measure_state { ms.level = -20.0; }
                        }))
                         .child(self.render_option_chip("-40 dB", (measure_state.level - (-40.0)).abs() < 0.1, cx, |state, val| {
                            if let Some(ms) = &mut state.app.measure_state { ms.level = -40.0; }
                        }))
                    )
            )
    }

    fn render_measure_running(
        &self,
        measure_state: &MeasureState,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Lg)
            .align(StackAlign::Center)
            .justify(StackJustify::Center)
            .child(Text::new("Measuring...").size(TextSize::Xl))
            .child(
                div()
                    .w(px(300.0))
                    .h(px(8.0))
                    .bg(theme.surface)
                    .rounded_full()
                    .child(
                        div()
                            .h_full()
                            .w(px(300.0 * measure_state.progress))
                            .bg(theme.accent)
                            .rounded_full()
                    )
            )
            .child(Text::new(&measure_state.status_message).muted(true))
    }

    fn render_measure_results(
        &self,
        measure_state: &MeasureState,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        if let Some(res) = &measure_state.measurement_result {
             div().w_full().child(
                 VStack::new()
                     .spacing(StackSpacing::Md)
                     .child(Text::new("Measurement Complete").weight(TextWeight::Bold).size(TextSize::Lg))
                     .child(Text::new(format!("Captured {} points.", res.frequencies.len())).muted(true))
                     .child(
                         div()
                            .h(px(250.0))
                            .w_full()
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.border)
                            .rounded_md()
                            .child(self.render_measurement_preview(res, theme))
                     )
                     .child(Text::new(&res.csv_path).size(TextSize::Xs).muted(true))
             ).into_any_element()
        } else {
             Text::new("No results available.").into_any_element()
        }
    }

    fn render_measurement_preview(&self, result: &crate::app::types::MeasurementResult, theme: &crate::theme::Theme) -> impl IntoElement {
         let width = 600.0;
         let height = 250.0;
         
         // Basic auto-ranging
         let min_db = result.magnitude_db.iter().fold(f32::INFINITY, |a, &b| a.min(b)) as f64 - 5.0;
         let max_db = result.magnitude_db.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b)) as f64 + 5.0;
         
         let freq_scale = LogScale::new().domain(20.0, 20000.0).range(0.0, width);
         let db_scale = LinearScale::new().domain(min_db, max_db).range(height, 0.0);
         
         let points: Vec<LinePoint> = result.frequencies.iter().zip(result.magnitude_db.iter())
            .map(|(&f, &db)| LinePoint::new(f as f64, db as f64)).collect();
            
         let config = LineConfig::new()
            .stroke_width(2.0)
            .stroke_color(D3Color::from_rgba(theme.accent));
            
         div()
            .relative()
            .size_full()
            .overflow_hidden()
            .child(render_line(&freq_scale, &db_scale, &points, &config))
            .child(
                div().absolute().bottom(px(0.0)).left(px(0.0)).text_xs().text_color(theme.text_muted).child("20Hz")
            )
            .child(
                div().absolute().bottom(px(0.0)).right(px(0.0)).text_xs().text_color(theme.text_muted).child("20kHz")
            )
            .child(
                 div().absolute().top(px(0.0)).left(px(0.0)).text_xs().text_color(theme.text_muted).child(format!("{:.1}dB", max_db))
            )
            .child(
                 div().absolute().bottom(px(0.0)).left(px(0.0)).mb(px(15.0)).text_xs().text_color(theme.text_muted).child(format!("{:.1}dB", min_db))
            )
    }

    fn render_measure_footer(
        &self,
        measure_state: &MeasureState,
        theme: &crate::theme::Theme,
         cx: &mut Context<Self>,
    ) -> impl IntoElement {
         HStack::new()
            .spacing(StackSpacing::Md)
            .justify(StackJustify::End)
            .child(
                Button::new("meas_cancel", "Cancel")
                    .variant(ButtonVariant::Ghost)
                    .build()
                    .on_mouse_up(MouseButton::Left, cx.listener(|view, _, _, cx| {
                        view.state.update(cx, |state, _| {
                            state.app.measure_state = None; // Close dialog
                        });
                    }))
            )
            .child(
                match measure_state.step {
                    MeasureStep::DeviceSelection => {
                         Button::new("meas_next_1", "Next")
                            .variant(ButtonVariant::Primary)
                            .build()
                            .on_mouse_up(MouseButton::Left, cx.listener(|view, _, _, cx| {
                                view.state.update(cx, |state, _| {
                                    if let Some(ms) = &mut state.app.measure_state {
                                        ms.step = MeasureStep::SignalConfig;
                                    }
                                });
                            })).into_any_element()
                    },
                    MeasureStep::SignalConfig => {
                         Button::new("meas_start", "Start Measurement")
                            .variant(ButtonVariant::Primary)
                             // Logic to start measurement
                            .build()
                            .on_mouse_up(MouseButton::Left, cx.listener(|view, _, _, cx| {
                                // Start the measurement task!
                                view.start_measurement_task(cx);
                            })).into_any_element()
                    },
                    MeasureStep::Running => {
                        div().into_any_element() // No buttons while running
                    },
                    MeasureStep::Results => {
                         Button::new("meas_save", "Save & Close")
                            .variant(ButtonVariant::Primary)
                            .build()
                            .on_mouse_up(MouseButton::Left, cx.listener(|view, _, _, cx| {
                                view.state.update(cx, |state, _| {
                                    state.app.measure_state = None; // Close and save (TODO: Actual Save)
                                });
                            })).into_any_element()
                    }
                }
            )
    }
    
    // Helper for chips (copied/adapted from screens/settings/headphone.rs if needed, or inline)
    fn render_option_chip<F>(
        &self,
        label: &str,
        selected: bool,
        cx: &mut Context<Self>,
        on_click: F,
    ) -> impl IntoElement
    where
        F: Fn(&mut crate::app::state::AppState, &bool) + 'static + Send + Sync,
    {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        
        div()
            .px_3()
            .py_1()
            .rounded_full()
            .text_sm()
            .cursor_pointer()
            .border_1()
            .when(selected, |d| {
                d.bg(theme.accent).text_color(theme.text_on_accent).border_color(theme.accent)
            })
            .when(!selected, |d| {
                d.bg(theme.surface).text_color(theme.text_primary).border_color(theme.border)
                 .hover(|s| s.bg(theme.surface_hover))
            })
            .child(label.to_string())
            .child(label.to_string())
            .on_mouse_up(MouseButton::Left, cx.listener(move |view, _, _, cx| {
                 view.state.update(cx, |state, _cx| {
                     on_click(state, &true);
                 });
            }))
    }
    
    // Placeholder for actual task start
    pub fn start_measurement_task(&self, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        // Safely extract params. If measure_state is None, return early (shouldn't happen)
        let (signal_str, duration, out_ch, in_ch, out_dev, in_dev, sample_rate) = if let Some(ms) = &state.app.measure_state {
            let out_idx = state.app.selected_output_device_index;
            let sr = state.app.output_devices.get(out_idx)
                .and_then(|d| d.default_config.as_ref())
                .map(|c| c.sample_rate)
                .unwrap_or(48000);
            
            (
                ms.signal_type.clone(),
                ms.duration.parse::<f32>().unwrap_or(5.0),
                ms.output_channel as u16,
                ms.input_channel as u16,
                state.app.current_output_device_name.clone(),
                state.app.current_input_device_name.clone(),
                sr
            )
        } else {
            return;
        };

        let state = self.state.clone();
        let executor = cx.background_executor().clone();
        
        cx.spawn(async move |_, cx| {
            // Update UI to Running state
            let _ = state.update(&mut cx.clone(), |state, _| {
                if let Some(ms) = &mut state.app.measure_state {
                    ms.step = MeasureStep::Running;
                    ms.progress = 0.0;
                    ms.status_message = "Generating signal...".to_string();
                }
            });

            // Perform measurement in background to avoid blocking UI
            let result = executor.spawn(async move {
                use sotf_audio_player::signal_recorder::*;
                use std::str::FromStr;

                // 1. Generate Signal
                let sig_type = SignalType::from_str(&signal_str).unwrap_or(SignalType::Sweep);
                let params = match sig_type {
                    SignalType::Sweep => SignalParams::Sweep { start_freq: 20.0, end_freq: 20000.0, amp: 0.5 },
                    _ => SignalParams::Noise { amp: 0.5 },
                };
                
                let base_signal = generate_signal(sig_type, &params, duration, sample_rate)
                    .map_err(|e| format!("Signal gen error: {}", e))?;
                
                let prep_signal = prepare_signal(base_signal, sample_rate);
                
                // 2. Write temp file
                let temp_wav = write_temp_wav(&prep_signal, sample_rate, 1)
                    .map_err(|e| format!("Temp file error: {}", e))?;
                
                let temp_dir = std::env::temp_dir();
                // Timestamp to avoid collisions?
                let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                let output_wav = temp_dir.join(format!("sotf_rec_{}.wav", ts));
                let output_csv = temp_dir.join(format!("sotf_rec_{}.csv", ts));
                
                // 3. Record
                record_and_analyze(
                    temp_wav.path(),
                    &output_wav,
                    &prep_signal,
                    sample_rate,
                    &output_csv,
                    out_ch,
                    in_ch,
                    out_dev.as_deref(),
                    in_dev.as_deref(),
                    None // No mic comp yet
                ).map_err(|e| format!("Recording error: {}", e))?;
                
                Ok::<_, String>((output_csv, output_wav))
            }).await;

            // Handle result
            match result {
                Ok((csv_path, _wav_path)) => {
                    // Success
                     let _ = state.update(cx, |state, _| {
                        if let Some(ms) = &mut state.app.measure_state {
                            ms.step = MeasureStep::Results;
                            ms.status_message = "Analysis complete.".to_string();
                            ms.progress = 1.0;
                                
                                // Parse CSV
                                let csv_content = std::fs::read_to_string(&csv_path).unwrap_or_default();
                                let mut frequencies = Vec::new();
                                let mut magnitude_db = Vec::new();
                                let mut phase_deg = Vec::new();
                                
                                for line in csv_content.lines().skip(1) { // Skip header
                                    let parts: Vec<&str> = line.split(',').collect();
                                    if parts.len() >= 2 {
                                        if let (Ok(f), Ok(db)) = (parts[0].parse::<f32>(), parts[1].parse::<f32>()) {
                                            frequencies.push(f);
                                            magnitude_db.push(db);
                                            if parts.len() >= 3 {
                                                if let Ok(p) = parts[2].parse::<f32>() {
                                                    phase_deg.push(p);
                                                } else {
                                                     phase_deg.push(0.0);
                                                }
                                            } else {
                                                 phase_deg.push(0.0);
                                            }
                                        }
                                    }
                                }
                                
                                use crate::app::types::MeasurementResult;
                                let result = MeasurementResult {
                                    frequencies,
                                    magnitude_db,
                                    phase_deg,
                                    csv_path: csv_path.to_string_lossy().to_string(),
                                };
                                
                                ms.measurement_result = Some(result);
                            }
                        });
                },
                Err(e) => {
                    // Error
                     let _ = state.update(cx, |state, _| {
                        if let Some(ms) = &mut state.app.measure_state {
                            ms.step = MeasureStep::Running; // Stay on running or go back?
                            ms.status_message = format!("Error: {}", e);
                            ms.progress = 0.0;
                        }
                     });
                }
            }
        }).detach();
    }
}
