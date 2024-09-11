use std::sync::Once;
use std::thread;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::oneshot;
use windows::Win32::System::Com::{CoCreateInstance, CoInitialize, CLSCTX_ALL};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern, IUIAutomationTreeWalker, TreeScope_Descendants, UIA_TextPatternId};

static INIT: Once = Once::new();

#[derive(Error, Debug)]
pub enum GetTextError {
    #[error("COM initialization failed: {0}")]
    ComInitError(#[from] windows::core::Error),
    #[error("UI Automation error: {0}")]
    UIAutomationError(String),
    #[error("Timeout while getting text")]
    Timeout,
}

pub async fn async_get_text() -> Result<String, GetTextError> {
    let (tx, rx) = oneshot::channel();

    thread::spawn(move || {
        let result = get_text();
        let _ = tx.send(result);
    });

    tokio::time::timeout(Duration::from_secs(1), rx)
        .await
        .map_err(|_| GetTextError::Timeout)?
        .map_err(|_| GetTextError::UIAutomationError("Channel send error".to_string()))?
}

fn get_text() -> Result<String, GetTextError> {
    unsafe { internal_get_text() }
}

unsafe fn internal_get_text() -> Result<String, GetTextError> {
    INIT.call_once(|| {
        let _ = CoInitialize(None);
    });

    let auto: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)?;
    let condition = auto.CreateTrueCondition()
        .map_err(|e| GetTextError::UIAutomationError(format!("Failed to create condition: {}", e)))?;
    let walker: IUIAutomationTreeWalker = auto.CreateTreeWalker(&condition)
        .map_err(|e| GetTextError::UIAutomationError(format!("Failed to create tree walker: {}", e)))?;

    let el = auto.GetFocusedElement()
        .map_err(|e| GetTextError::UIAutomationError(format!("Failed to get focused element: {}", e)))?;

    // Check focused
    if let Some(text) = get_text_from_element(&el) {
        return Ok(text);
    }

    // Check next sibling
    if let Ok(sibling) = walker.GetNextSiblingElement(&el) {
        if let Some(text) = get_text_from_element(&sibling) {
            return Ok(text);
        }
    }

    // Check prev sibling
    if let Ok(sibling) = walker.GetPreviousSiblingElement(&el) {
        if let Some(text) = get_text_from_element(&sibling) {
            return Ok(text);
        }
    }

    // Check descendants
    const MAX_DESCENDANTS_TO_CHECK: i32 = 50;

    if let Ok(descendants) = el.FindAll(TreeScope_Descendants, &condition) {
        let length = descendants.Length()?;
        for i in 0..length {
            if let Ok(descendant) = descendants.GetElement(i) {
                if let Some(text) = get_non_empty_text(&descendant) {
                    return Ok(text);
                }
            }
            if i >= MAX_DESCENDANTS_TO_CHECK - 1 {
                break;
            }
        }
    }

    // Check parents
    const MAX_PARENTS_TO_CHECK: i32 = 50;
    let mut current_element = el;

    for _ in 0..MAX_PARENTS_TO_CHECK {
        match walker.GetParentElement(&current_element) {
            Ok(parent) => {
                if let Some(text) = get_non_empty_text(&parent) {
                    return Ok(text);
                }
                current_element = parent;
            }
            Err(_) => break,
        }
    }

    Ok(String::new())
}

fn get_non_empty_text(el: &IUIAutomationElement) -> Option<String> {
    get_text_from_element(el).filter(|s| !s.is_empty())
}

fn get_text_from_element(el: &IUIAutomationElement) -> Option<String> {
    unsafe {
        el.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
            .ok()
            .and_then(|pattern| {
                pattern.GetSelection().ok().and_then(|text_array| {
                    text_array.Length().ok().and_then(|length| {
                        (0..length).try_fold(String::new(), |mut acc, i| {
                            text_array.GetElement(i)
                                .and_then(|text| text.GetText(-1))
                                .map(|str| {
                                    acc.push_str(&str.to_string());
                                    acc
                                })
                                .ok()
                        })
                    })
                })
            })
            .map(String::from)
    }
}