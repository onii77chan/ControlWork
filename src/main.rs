use anyhow::{Context, Result};
use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use teloxide::{
    dispatching::dialogue::InMemStorage,
    net::Download,
    payloads::SendMessageSetters,
    prelude::*,
    types::{InputFile, MessageId},
    utils::command::BotCommands,
};
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};
use tokio::fs;
use tokio::process::Command;
use tokio::sync::Mutex;
use uuid::Uuid;
use log::{info, error};

// Карта форматов: что во что можно конвертировать
lazy_static! {
    static ref FORMAT_MAP: HashMap<&'static str, Vec<&'static str>> = {
        let mut m = HashMap::new();
        // Изображения (через crate image)
        m.insert("png", vec!["jpg", "jpeg", "webp", "ico", "bmp", "gif"]);
        m.insert("jpg", vec!["png", "webp", "ico", "bmp", "gif"]);
        m.insert("jpeg", vec!["png", "webp", "ico", "bmp", "gif"]);
        m.insert("webp", vec!["png", "jpg", "jpeg", "ico", "bmp", "gif"]);
        m.insert("bmp", vec!["png", "jpg", "jpeg", "webp", "ico", "gif"]);
        m.insert("gif", vec!["png", "jpg", "jpeg", "webp", "ico", "bmp"]);

        // Аудио/Видео (через ffmpeg)
        m.insert("mp4", vec!["mp3", "wav", "ogg", "gif"]);
        m.insert("wav", vec!["mp3", "ogg"]);
        m.insert("ogg", vec!["mp3", "wav"]);
        m.insert("mp3", vec!["wav", "ogg"]);
        m.insert("m4a", vec!["mp3", "wav", "ogg"]);
        m.insert("webm", vec!["mp4", "mp3"]);

        // Документы (через Python convert.py / pandoc / mammoth + xhtml2pdf)
        m.insert("pdf", vec!["docx"]);
        m.insert("docx", vec!["pdf"]);
        m.insert("md", vec!["docx", "html", "pdf"]);
        m.insert("html", vec!["md", "docx"]);

        m
    };
}

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
    ReceiveFiles,
    WaitFormatSelection {
        files: Vec<FileInfo>,
        possible_formats: Vec<String>,
        group_id: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub struct FileInfo {
    pub file_id: String,
    pub file_name: String,
    pub ext: String,
    pub message_id: MessageId,
}

type MyDialogue = Dialogue<State, InMemStorage<State>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

// Временное хранилище для медиагрупп
type MediaGroupCache = Arc<Mutex<HashMap<String, Vec<FileInfo>>>>;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Поддерживаемые команды:")]
enum CommandCmd {
    #[command(description = "Начать работу с ботом")]
    Start,
    #[command(description = "Отменить текущую операцию")]
    Cancel,
}

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();
    info!("Starting Universal Converter Bot...");

    // Создаем директорию temp, если её нет
    if !Path::new("temp").exists() {
        fs::create_dir("temp").await.context("Не удалось создать директорию temp")?;
    }

    let bot = Bot::from_env();
    let media_group_cache: MediaGroupCache = Arc::new(Mutex::new(HashMap::new()));

    let handler = Update::filter_message()
        .enter_dialogue::<Message, InMemStorage<State>, State>()
        .branch(dptree::entry().filter_command::<CommandCmd>().endpoint(handle_commands))
        .branch(dptree::case![State::Start].endpoint(handle_files))
        .branch(dptree::case![State::ReceiveFiles].endpoint(handle_files));

    let callback_handler = Update::filter_callback_query()
        .enter_dialogue::<CallbackQuery, InMemStorage<State>, State>()
        .branch(dptree::case![State::WaitFormatSelection { files, possible_formats, group_id }].endpoint(handle_format_selection));

    let main_handler = dptree::entry()
        .branch(handler)
        .branch(callback_handler);

    Dispatcher::builder(bot, main_handler)
        .dependencies(dptree::deps![InMemStorage::<State>::new(), media_group_cache])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

async fn handle_commands(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    cmd: CommandCmd,
) -> HandlerResult {
    match cmd {
        CommandCmd::Start => {
            bot.send_message(msg.chat.id, "Привет! Я универсальный конвертер файлов.\n\nОтправь мне файл, фото, аудио или видео, и я предложу форматы для конвертации. Ты можешь отправить несколько файлов сразу (альбомом).").await?;
            dialogue.update(State::ReceiveFiles).await?;
        }
        CommandCmd::Cancel => {
            bot.send_message(msg.chat.id, "Операция отменена. Жду новые файлы.").await?;
            dialogue.update(State::ReceiveFiles).await?;
        }
    }
    Ok(())
}

fn extract_file_info(msg: &Message) -> Option<FileInfo> {
    if let Some(doc) = msg.document() {
        let name = doc.file_name.clone().unwrap_or_else(|| "file".to_string());
        let ext = name.split('.').last().unwrap_or("").to_lowercase();
        Some(FileInfo {
            file_id: doc.file.id.clone(),
            file_name: name,
            ext,
            message_id: msg.id,
        })
    } else if let Some(photos) = msg.photo() {
        if let Some(photo) = photos.last() {
            Some(FileInfo {
                file_id: photo.file.id.clone(),
                file_name: "image.jpg".to_string(),
                ext: "jpg".to_string(),
                message_id: msg.id,
            })
        } else {
            None
        }
    } else if let Some(audio) = msg.audio() {
        let name = audio.file_name.clone().unwrap_or_else(|| "audio.mp3".to_string());
        let ext = name.split('.').last().unwrap_or("mp3").to_lowercase();
        Some(FileInfo {
            file_id: audio.file.id.clone(),
            file_name: name,
            ext,
            message_id: msg.id,
        })
    } else if let Some(video) = msg.video() {
        let name = video.file_name.clone().unwrap_or_else(|| "video.mp4".to_string());
        let ext = name.split('.').last().unwrap_or("mp4").to_lowercase();
        Some(FileInfo {
            file_id: video.file.id.clone(),
            file_name: name,
            ext,
            message_id: msg.id,
        })
    } else if let Some(voice) = msg.voice() {
        Some(FileInfo {
            file_id: voice.file.id.clone(),
            file_name: "voice.ogg".to_string(),
            ext: "ogg".to_string(),
            message_id: msg.id,
        })
    } else {
        None
    }
}

async fn handle_files(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    cache: MediaGroupCache,
) -> HandlerResult {
    let file_info = match extract_file_info(&msg) {
        Some(info) => info,
        None => {
            bot.send_message(msg.chat.id, "Пожалуйста, отправь мне файл (документ, фото, аудио или видео).").await?;
            return Ok(());
        }
    };

    let group_id = msg.media_group_id().map(|s| s.to_string());

    if let Some(ref gid) = group_id {
        // Добавляем файл в кэш
        let mut cache_lock = cache.lock().await;
        cache_lock.entry(gid.clone()).or_insert_with(Vec::new).push(file_info.clone());
        let _count = cache_lock.get(gid).unwrap().len();
        drop(cache_lock);

        // Ждем немного, чтобы собрать все файлы из альбома
        let bot_clone = bot.clone();
        let chat_id = msg.chat.id;
        let dialogue_clone = dialogue.clone();
        let cache_clone = cache.clone();
        let gid_clone = gid.clone();

        // Запускаем таску ожидания
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1500)).await;

            let mut cache_lock = cache_clone.lock().await;
            if let Some(files) = cache_lock.remove(&gid_clone) {
                // Если мы здесь, значит время вышло, и мы можем обработать альбом
                drop(cache_lock);
                process_files_for_format_selection(bot_clone, chat_id, dialogue_clone, files, Some(gid_clone)).await.unwrap_or_else(|e| {
                    error!("Error processing media group: {}", e);
                });
            }
        });

    } else {
        // Одиночный файл
        process_files_for_format_selection(bot, msg.chat.id, dialogue, vec![file_info], None).await?;
    }

    Ok(())
}

async fn process_files_for_format_selection(
    bot: Bot,
    chat_id: teloxide::types::ChatId,
    dialogue: MyDialogue,
    files: Vec<FileInfo>,
    group_id: Option<String>,
) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }

    // Определяем пересечение доступных форматов для всех файлов
    let mut possible_formats: Option<HashSet<&str>> = None;

    for file in &files {
        if let Some(formats) = FORMAT_MAP.get(file.ext.as_str()) {
            let formats_set: HashSet<&str> = formats.iter().copied().collect();
            match possible_formats {
                None => possible_formats = Some(formats_set),
                Some(ref mut current) => {
                    *current = current.intersection(&formats_set).copied().collect();
                }
            }
        } else {
             // Если хотя бы для одного файла нет форматов, очищаем пересечение
             possible_formats = Some(HashSet::new());
             break;
        }
    }

    let formats: Vec<String> = possible_formats
        .unwrap_or_default()
        .into_iter()
        .map(String::from)
        .collect();

    if formats.is_empty() {
        bot.send_message(chat_id, "Извините, не нашел общих форматов для конвертации этих файлов.").await?;
        return Ok(());
    }

    // Создаем inline клавиатуру
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    let mut row: Vec<InlineKeyboardButton> = Vec::new();

    for format in &formats {
        row.push(InlineKeyboardButton::callback(format.to_uppercase(), format.clone()));
        if row.len() == 3 {
            keyboard.push(row);
            row = Vec::new();
        }
    }
    if !row.is_empty() {
        keyboard.push(row);
    }

    keyboard.push(vec![InlineKeyboardButton::callback("❌ Отмена", "cancel")]);

    let text = if files.len() == 1 {
        format!("Файл: {}\nВыбери целевой формат:", files[0].file_name)
    } else {
        format!("Получено файлов: {}\nВыбери целевой формат для всех файлов:", files.len())
    };

    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(keyboard))
        .await?;

    dialogue.update(State::WaitFormatSelection { files, possible_formats: formats, group_id }).await?;

    Ok(())
}

async fn handle_format_selection(
    bot: Bot,
    q: CallbackQuery,
    dialogue: MyDialogue,
    (files, possible_formats, _group_id): (Vec<FileInfo>, Vec<String>, Option<String>),
) -> HandlerResult {
    let target_format = match q.data {
        Some(data) => data,
        None => return Ok(()),
    };

    if let Some(msg) = q.message {
        bot.delete_message(msg.chat.id, msg.id).await?;

        if target_format == "cancel" {
            bot.send_message(msg.chat.id, "Конвертация отменена.").await?;
            dialogue.update(State::ReceiveFiles).await?;
            return Ok(());
        }

        if !possible_formats.contains(&target_format) {
            bot.send_message(msg.chat.id, "Недопустимый формат.").await?;
            return Ok(());
        }

        let processing_msg = bot.send_message(msg.chat.id, format!("Начинаю конвертацию {} файлов в {}...", files.len(), target_format.to_uppercase())).await?;

        // Запускаем процесс конвертации
        let mut converted_files: Vec<PathBuf> = Vec::new();
        let mut temp_paths_to_cleanup: Vec<PathBuf> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        for file_info in files {
            let bot_clone = bot.clone();

            // Скачивание файла
            let tg_file = match bot_clone.get_file(&file_info.file_id).await {
                Ok(f) => f,
                Err(e) => {
                    errors.push(format!("Не удалось получить инфо о файле {}: {}", file_info.file_name, e));
                    continue;
                }
            };

            let uuid_str = Uuid::new_v4().to_string();
            let input_path = PathBuf::from("temp").join(format!("{}.{}", uuid_str, file_info.ext));
            temp_paths_to_cleanup.push(input_path.clone());

            let mut out_file = match fs::File::create(&input_path).await {
                Ok(f) => f,
                Err(e) => {
                    errors.push(format!("Не удалось создать временный файл: {}", e));
                    continue;
                }
            };

            if let Err(e) = bot_clone.download_file(&tg_file.path, &mut out_file).await {
                errors.push(format!("Не удалось скачать файл {}: {}", file_info.file_name, e));
                continue;
            }

            // Явно закрываем файл перед вызовом внешних процессов
            drop(out_file);

            // Имя выходного файла: заменяем расширение
            let base_name = Path::new(&file_info.file_name).file_stem().unwrap_or_default().to_string_lossy();
            let safe_base_name = base_name.replace(" ", "_"); // Basic sanitization
            let output_name = format!("{}.{}", safe_base_name, target_format);
            let output_path = PathBuf::from("temp").join(format!("{}_{}", uuid_str, output_name));
            temp_paths_to_cleanup.push(output_path.clone());

            // Вызов конвертера
            match convert_file(&input_path, &output_path, &file_info.ext, &target_format).await {
                Ok(_) => {
                    converted_files.push(output_path);
                }
                Err(e) => {
                    errors.push(format!("Ошибка конвертации {}: {}", file_info.file_name, e));
                }
            }
        }

        // Отправка результатов
        let mut send_error = None;
        if !converted_files.is_empty() {
             bot.delete_message(msg.chat.id, processing_msg.id).await.ok();

             // Если файл один
             if converted_files.len() == 1 {
                 let path = &converted_files[0];
                 let file_name = path.file_name().unwrap().to_string_lossy().to_string();
                 // Извлекаем оригинальное имя файла (без uuid_)
                 let display_name = file_name.split('_').skip(1).collect::<Vec<&str>>().join("_");

                 if let Err(e) = bot.send_document(msg.chat.id, InputFile::file(path).file_name(display_name)).await {
                     send_error = Some(e);
                 }
             } else {
                 // Если файлов несколько, отправляем по одному
                 for path in &converted_files {
                     let file_name = path.file_name().unwrap().to_string_lossy().to_string();
                     let display_name = file_name.split('_').skip(1).collect::<Vec<&str>>().join("_");

                     if let Err(e) = bot.send_document(msg.chat.id, InputFile::file(path).file_name(display_name)).await {
                         send_error = Some(e);
                         break; // Если одна ошибка, прерываем цикл
                     }
                 }
             }
        } else {
            if let Err(e) = bot.edit_message_text(msg.chat.id, processing_msg.id, "Не удалось конвертировать ни один файл.").await {
                send_error = Some(e);
            }
        }

        if !errors.is_empty() {
            let error_msg = errors.join("\n");
            if let Err(e) = bot.send_message(msg.chat.id, format!("Некоторые ошибки:\n{}", error_msg)).await {
                // Если не удалось отправить сообщение об ошибке, просто логируем
                error!("Failed to send error message: {}", e);
            }
        }

        // Очистка временных файлов
        for path in temp_paths_to_cleanup {
            let _ = fs::remove_file(path).await;
        }

        // Теперь возвращаем ошибку отправки, если она была, после очистки
        if let Some(e) = send_error {
            return Err(e.into());
        }

        dialogue.update(State::ReceiveFiles).await?;
    }

    // Ответ на callback (чтобы крутилка на кнопке исчезла)
    bot.answer_callback_query(&q.id).await?;

    Ok(())
}

async fn convert_file(input: &Path, output: &Path, ext_in: &str, ext_out: &str) -> Result<()> {
    let ext_in = ext_in.to_lowercase();
    let ext_out = ext_out.to_lowercase();

    // 1. Изображения: нативная конвертация через crate image (если возможно)
    let img_exts = ["png", "jpg", "jpeg", "webp", "bmp", "ico", "gif"];
    if img_exts.contains(&ext_in.as_str()) && img_exts.contains(&ext_out.as_str()) {
        info!("Converting image natively from {} to {}", ext_in, ext_out);

        let input_path = input.to_path_buf();
        let output_path = output.to_path_buf();
        let ext_out_owned = ext_out.to_string();

        let res = tokio::task::spawn_blocking(move || -> Result<()> {
            let img = image::open(&input_path).context("Failed to open image")?;

            let format = match ext_out_owned.as_str() {
                "png" => image::ImageFormat::Png,
                "jpg" | "jpeg" => image::ImageFormat::Jpeg,
                "webp" => image::ImageFormat::WebP,
                "bmp" => image::ImageFormat::Bmp,
                "ico" => image::ImageFormat::Ico,
                "gif" => image::ImageFormat::Gif,
                _ => image::ImageFormat::Png,
            };

            img.save_with_format(&output_path, format).context("Failed to save image")?;
            Ok(())
        }).await.context("Task spawned blocking panicked")?;

        return res;
    }

    // 2. Медиа: конвертация через ffmpeg
    let media_exts = ["mp4", "mp3", "wav", "ogg", "m4a", "webm"];
    if media_exts.contains(&ext_in.as_str()) || media_exts.contains(&ext_out.as_str()) {
        info!("Converting media using ffmpeg from {} to {}", ext_in, ext_out);
        let status = Command::new("ffmpeg")
            .arg("-y") // Overwrite output files without asking
            .arg("-i")
            .arg(input)
            .arg(output)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .context("Failed to execute ffmpeg")?;

        if status.success() {
            return Ok(());
        } else {
            return Err(anyhow::anyhow!("ffmpeg failed with status: {}", status));
        }
    }

    // 3. Документы: вызов Python скрипта
    info!("Converting document using python script from {} to {}", ext_in, ext_out);
    let status = Command::new("python3")
        .arg("convert.py")
        .arg(input)
        .arg(output)
        .arg(&ext_out)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await
        .context("Failed to execute python3 convert.py")?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("python script failed with status: {}", status))
    }
}
