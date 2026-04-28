//! `/honeybot setup|lang|notif` — per-guild config that doesn't belong to a
//! single feature module. `setup` is the one-shot first-run command;
//! `lang` and `notif` are the same fields editable individually later.

use anyhow::{Context, Result, anyhow};
use std::sync::Arc;
use twilight_model::application::command::{Command, CommandType};
use twilight_model::application::interaction::Interaction;
use twilight_model::application::interaction::application_command::{
    CommandData, CommandDataOption, CommandOptionValue,
};
use twilight_model::id::Id;
use twilight_model::id::marker::{ApplicationMarker, GuildMarker};
use twilight_util::builder::command::{
    ChannelBuilder, CommandBuilder, StringBuilder, SubCommandBuilder,
};

use crate::bot::AppState;
use crate::commands::util::{
    option_channel, option_channel_opt, option_string, option_string_opt, reply,
};

const SUPPORTED_LOCALES: [(&str, &str); 2] = [("English", "en"), ("Français", "fr")];

pub fn definition() -> Command {
    let lang_choices: Vec<(&str, String)> = SUPPORTED_LOCALES
        .iter()
        .map(|(label, code)| (*label, (*code).to_owned()))
        .collect();
    // Same list, cloned so the two builder calls each take ownership.
    let setup_lang_choices = lang_choices.clone();

    CommandBuilder::new(
        "honeybot",
        "Configure honeybot for this server.",
        CommandType::ChatInput,
    )
    .option(
        // Both options are optional: bare `/honeybot setup` defaults to
        // English and uses the channel the command was invoked from as the
        // notification channel. The shortest path to a working bot is then:
        //   /honeybot setup
        //   /honeypot add #trap ban
        SubCommandBuilder::new(
            "setup",
            "Initial config. Defaults to English + the current channel for notifications.",
        )
        .option(
            StringBuilder::new("language", "Locale for bot messages (default: English).")
                .choices(setup_lang_choices),
        )
        .option(ChannelBuilder::new(
            "notification_channel",
            "Channel where the bot posts triggers and escalations (default: this channel).",
        )),
    )
    .option(
        SubCommandBuilder::new("lang", "Change the locale for bot messages.").option(
            StringBuilder::new("language", "Locale code.")
                .required(true)
                .choices(lang_choices),
        ),
    )
    .option(
        SubCommandBuilder::new("notif", "Change the notification channel.")
            .option(ChannelBuilder::new("channel", "New notification channel.").required(true)),
    )
    .build()
}

pub async fn handle(
    state: Arc<AppState>,
    application_id: Id<ApplicationMarker>,
    interaction: Interaction,
    command: CommandData,
) -> Result<()> {
    let Some(guild_id) = interaction.guild_id else {
        return reply(
            &state,
            application_id,
            &interaction,
            "This command can only be used inside a server.",
        )
        .await;
    };

    let sub = command
        .options
        .first()
        .ok_or_else(|| anyhow!("missing subcommand"))?;
    let CommandOptionValue::SubCommand(opts) = &sub.value else {
        return Err(anyhow!("expected subcommand value"));
    };

    let invoking_channel = interaction.channel.as_ref().map(|c| c.id);

    let content = match sub.name.as_str() {
        "setup" => setup(&state, guild_id, invoking_channel, opts).await?,
        "lang" => lang(&state, guild_id, opts).await?,
        "notif" => notif(&state, guild_id, opts).await?,
        other => format!("Unknown subcommand `{other}`."),
    };

    reply(&state, application_id, &interaction, &content).await
}

async fn setup(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    invoking_channel: Option<Id<twilight_model::id::marker::ChannelMarker>>,
    options: &[CommandDataOption],
) -> Result<String> {
    let language = option_string_opt(options, "language").unwrap_or_else(|| "en".to_owned());
    let channel = option_channel_opt(options, "notification_channel")
        .or(invoking_channel)
        .ok_or_else(|| {
            anyhow!("could not infer a notification channel; pass `notification_channel:`")
        })?;

    sqlx::query(
        "INSERT INTO guilds (id, locale, notification_channel_id)
         VALUES (?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            locale = excluded.locale,
            notification_channel_id = excluded.notification_channel_id",
    )
    .bind(guild_id.get() as i64)
    .bind(&language)
    .bind(channel.get() as i64)
    .execute(&state.db)
    .await
    .context("upsert guild config")?;

    Ok(format!(
        "Setup complete. Locale: `{language}`. Notifications go to <#{}>.\n\
         Next: `/honeypot add #channel ban` to configure your first honeypot.",
        channel.get()
    ))
}

async fn lang(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    options: &[CommandDataOption],
) -> Result<String> {
    let language = option_string(options, "language")?;

    sqlx::query(
        "INSERT INTO guilds (id, locale) VALUES (?, ?)
         ON CONFLICT(id) DO UPDATE SET locale = excluded.locale",
    )
    .bind(guild_id.get() as i64)
    .bind(&language)
    .execute(&state.db)
    .await
    .context("update guild locale")?;

    Ok(format!("Locale set to `{language}`."))
}

async fn notif(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    options: &[CommandDataOption],
) -> Result<String> {
    let channel = option_channel(options, "channel")?;

    sqlx::query(
        "INSERT INTO guilds (id, notification_channel_id) VALUES (?, ?)
         ON CONFLICT(id) DO UPDATE SET notification_channel_id = excluded.notification_channel_id",
    )
    .bind(guild_id.get() as i64)
    .bind(channel.get() as i64)
    .execute(&state.db)
    .await
    .context("update notification channel")?;

    Ok(format!("Notification channel set to <#{}>.", channel.get()))
}
