//! The score: two numbers, kept as a resource, and the text that mirrors them.
//!
//! [`Ball`](crate::ball) is the only thing that ever calls [`Score::add`]; nothing here ever writes
//! to a [`TextSpan`] except [`redraw`], and only when the resource that drives it has actually
//! changed. A controller that reached into the text directly would work today and drift the moment
//! a second thing needed to know the score.

use bevy::prelude::*;

use super::paddle::Side;

#[derive(Resource, Default)]
pub struct Score {
    left: u32,
    right: u32,
}

impl Score {
    pub fn add(&mut self, side: Side) {
        if side == Side::LEFT {
            self.left += 1;
        } else {
            self.right += 1;
        }
    }
}

#[derive(Component, Clone, Default)]
struct LeftScore;

#[derive(Component, Clone, Default)]
struct RightScore;

pub fn plugin(app: &mut App) {
    app.init_resource::<Score>();
    app.add_systems(Startup, scoreboard.spawn());
    app.add_systems(Update, redraw);
}

/// A full-width bar rather than a centered node directly: `left: 50%` positions a node's own edge,
/// not its content, so a fixed-width node placed that way sits with its content starting at center
/// and running right, not straddling it. Centering the bar's content with `justify_content` instead
/// means nothing here has to know how wide the score text is.
fn scoreboard() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(16.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            justify_content: JustifyContent::Center,
        }
        Children [
            Text::new("")
            Children [
                LeftScore
                TextSpan::new("0")
                TextFont { font_size: 40.0_f32 }
                TextColor(Color::WHITE)
                --
                TextSpan::new("   ")
                TextFont { font_size: 40.0_f32 }
                TextColor(Color::WHITE)
                --
                RightScore
                TextSpan::new("0")
                TextFont { font_size: 40.0_f32 }
                TextColor(Color::WHITE)
            ]
        ]
    }
}

fn redraw(
    score: Res<Score>,
    mut left: Query<&mut TextSpan, (With<LeftScore>, Without<RightScore>)>,
    mut right: Query<&mut TextSpan, With<RightScore>>,
) {
    if !score.is_changed() {
        return;
    }
    for mut span in &mut left {
        span.0 = score.left.to_string();
    }
    for mut span in &mut right {
        span.0 = score.right.to_string();
    }
}
