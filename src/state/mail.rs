use crate::{
    cfg::Settings,
    error::{VentError, LettreAction, LettreEmailSnafu},
};
use lettre::{
    transport::smtp::authentication::Credentials, AsyncSmtpTransport, AsyncTransport, Message,
    Tokio1Executor,
};
use snafu::ResultExt;
use tokio::sync::{
    broadcast::Receiver as BroadcastReceiver,
    mpsc::{unbounded_channel, UnboundedSender},
};

#[derive(Debug)]
pub struct EmailToSend {
    pub to_username: String,
    pub to_id: i32,
    pub to_fullname: String,
    pub unique_id: i32,
}

pub fn email_sender_thread(
    settings: Settings,
    mut stop_rx: BroadcastReceiver<()>,
) -> UnboundedSender<EmailToSend> {
    let mail_settings = settings.mail.clone();
    let (msg_tx, mut msg_rx) = unbounded_channel();

    async fn send_email(
        EmailToSend {
            to_username,
            to_id,
            to_fullname,
            unique_id,
        }: EmailToSend,
        mailer: &AsyncSmtpTransport<Tokio1Executor>,
        from_username: &str,
        username_domain: &str,
        project_name: &str,
        project_domain: &str,
    ) -> Result<(), VentError> {
        let m = Message::builder()
            .from(format!("{} NoReply <{}>", project_name, from_username).parse()?)
            .to(format!("{to_fullname} <{to_username}@{}>", username_domain).parse()?)
            .subject(format!("{} - Add Password", project_name))
            .body(format!(
                r#"Dear {},

You've just tried to login to {}, but you don't have a password set yet.

To set one, go to {}/add_password/{}?code={}.

Have a nice day!"#,
                to_fullname, project_name, project_domain, to_id, unique_id
            ))
            .context(LettreEmailSnafu {
                trying_to: LettreAction::BuildMessage,
            })?;

        info!(%to_fullname, %to_id, numbers=%unique_id, "Sending email.");

        mailer.send(m).await?;

        Ok(())
    }

    tokio::spawn(async move {
        let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&mail_settings.smtp)
            .expect("unable to get relay")
            .credentials(Credentials::new(
                mail_settings.username.clone(),
                mail_settings.password.clone(),
            ))
            .build();

        loop {
            if tokio::select! {
                _stop = stop_rx.recv() => {
                    info!("Mail thread stopping");
                    true
                },
                msg = msg_rx.recv() => match msg {
                    None => true,
                    Some(msg) => {
                        if let Err(e) = send_email(msg, &mailer, &mail_settings.username, &mail_settings.username_domain, &settings.brand.instance_name, &settings.brand.domain).await {
                            error!(?e, "Error sending email");
                        }

                        false
                    }
                }
            } {
                return;
            }
        }
    });

    msg_tx
}
