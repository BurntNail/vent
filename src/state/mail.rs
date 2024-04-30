use crate::{
    cfg::Settings,
    error::{LettreAction, LettreEmailSnafu, VentError},
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
            .from(format!("{project_name} NoReply <{from_username}>").parse()?)
            .to(format!("{to_fullname} <{to_username}@{username_domain}>").parse()?)
            .subject(format!("{project_name} - Add Password"))
            .body(format!(
                r#"Dear {to_fullname},

You've just tried to login to {project_name}, but you don't have a password set yet.

To set one, go to {project_domain}/add_password/{to_id}?code={unique_id}.

Have a nice day!"#
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
            if let Some(ret) = tokio::select! {
                _stop = stop_rx.recv() => {
                    info!("Mail thread stopping");
                    Some(Ok::<_, VentError>(()))
                },
                msg = msg_rx.recv() => match msg {
                    None => Some(Ok(())),
                    Some(msg) => {
                        if let Err(e) = send_email(msg, &mailer, &mail_settings.username, &mail_settings.username_domain, &settings.brand.instance_name, &settings.brand.domain).await {
                            error!(?e, "Error sending email");
                        }

                        None
                    }
                }
            } {
                return ret;
            }
        }
    });

    msg_tx
}
