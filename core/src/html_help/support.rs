//! The viewer's Web Help toolbar pages. The Windows 2000 directory's entries open
//! local support guidance, so its original Internet redirects are never required.

use crate::fb::{Framebuffer, Rect};
use crate::help::{Paragraph, Style, Topic};
use crate::theme::{Era, rgb};

const DIRECTORY: &[(&str, &str)] = &[
    (
        "Windows 2000 home page",
        "For an introduction to this program, choose Back and open the first topic on the Contents tab. To identify the Windows release you are using, open About Windows on that computer.",
    ),
    (
        "Get tips and practical advice about Windows 2000",
        "Search this book for the task you want to finish. Use the Index when you know a command or setting by name.",
    ),
    (
        "Contact product support",
        "For a problem with an application, contact its publisher. Include the program version, the steps that led to the problem, and the wording of any message.",
    ),
    (
        "Download updates, new features, and drivers from Windows Update",
        "Use the update tools for the Windows version installed on your computer. For a device driver, check the manufacturer and model before choosing a download.",
    ),
    (
        "Search for Windows 2000-compatible software",
        "Check the application's published system requirements. Look for the supported Windows versions and any required service pack.",
    ),
    (
        "Search for Windows 2000-compatible hardware",
        "Check the device maker's specifications and driver list. Match the model number and your installed Windows version.",
    ),
    (
        "Learn how to deploy Windows 2000",
        "Use the deployment guide for your particular Windows edition. Record the computer's hardware, installation media, and recovery options before planning an installation.",
    ),
    (
        "View Windows 2000 Resource Kit information",
        "If you have the Resource Kit for this edition, use its table of contents to find utilities, command syntax, and administration procedures.",
    ),
    (
        "Learn more about accessibility resources",
        "Look for Accessibility Options in the Windows Control Panel. The program's help index can also identify keyboard commands and alternatives to mouse input.",
    ),
];

pub(super) fn topics(era: Era, first: usize) -> Vec<Topic> {
    if era == Era::Win98 {
        let title = "Additional help via the Internet";
        let mut topic = Topic::from_paragraphs(
            title,
            &[
                (Style::Subhead, title),
                (Style::Body, "If the topic you need is missing from this book, try these other sources of help."),
                (
                    Style::Bullet,
                    "Look in the manual that came with the program or computer. The troubleshooting section often lists the same messages you see on screen.",
                ),
                (
                    Style::Bullet,
                    "For a question about a program, visit its publisher’s support site. For a device, start with the computer or device manufacturer's support pages.",
                ),
                (
                    Style::Bullet,
                    "When asking for support, include your version of Windows, what you were doing, and the exact message you saw.",
                ),
            ],
        );
        topic.welcome = true;
        return vec![topic];
    }
    let title = "Online support and information";
    let mut directory = Topic::from_paragraphs(title, &[]);
    let mut pages = Vec::with_capacity(DIRECTORY.len() + 1);
    for (i, &(label, guidance)) in DIRECTORY.iter().enumerate() {
        directory.paragraphs.push(Paragraph { style: Style::Link(first + i + 1), text: label.into() });
        pages.push(Topic::from_paragraphs(
            label,
            &[(Style::Title, label), (Style::Body, guidance), (Style::Link(first), title)],
        ));
    }
    pages.insert(0, directory);
    pages
}

/// Two cropped 4-bit sprites, the title (199×41) and the arrow beneath it (35×51).
/// The two solid bands to the right of the title extend to the pane's current width.
pub(super) fn paint_heading(fb: &mut Framebuffer, scroll: i32) {
    const DATA: &[u8; 4973] = crate::art::exactly("html-help/support-heading", crate::art::Look::Win2000);
    const PALETTE: &[u32] = crate::art::colors("html-help/support-heading-colors", crate::art::Look::Win2000);
    fb.fill(Rect::new(199, -scroll, fb.width - 199, 21), rgb(0));
    fb.fill(Rect::new(199, 21 - scroll, fb.width - 199, 20), rgb(0xff3300));
    let mut raw = DATA.as_slice();
    for r in [Rect::new(0, 0, 199, 41), Rect::new(26, 41, 35, 51)] {
        let count = (r.w * r.h) as usize;
        let (sprite, rest) = raw.split_at(count.div_ceil(2));
        raw = rest;
        for i in 0..count {
            let byte = sprite[i / 2];
            let color = if i % 2 == 0 { byte >> 4 } else { byte & 15 };
            fb.set(r.x + i as i32 % r.w, r.y + i as i32 / r.w - scroll, rgb(PALETTE[usize::from(color)]));
        }
    }
}
