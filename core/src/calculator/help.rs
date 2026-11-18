//! Calculator Help as printed in each edition's book: CALC.HLP on Windows 95 and calc.chm on
//! Windows 98 and 2000. Contents and index entries point into that same edition's topics.
use crate::help::{Catalog, Entry, EntryIcon, Keyword, Paragraph, Style, Topic};
use crate::theme::Era;
use Style::{Body, Bullet, Continuation, Link, Note, Related, Step, Subhead, Table, Title};

/// A button label and its equivalent key, spelled as printed in the book.
pub type KeyRow = (&'static str, &'static str);

#[rustfmt::skip]
const fn win9x_keys(backspace: &'static str) -> [KeyRow; 58] {
    [
        ("%", "%"), ("(", "("), (")", ")"), ("*", "*"), ("+", "+"), ("+/-", "F9"), ("-", "-"), (".", ". or ,"),
        ("/", "/"), ("0-9", "0-9"), ("1/x", "r"), ("=", "ENTER"), ("A-F", "A-F"), ("And", "&"), ("Ave", "CTRL+A"),
        ("Back", backspace), ("Bin", "F8"), ("Byte", "F4"), ("C", "ESC"), ("CE", "DEL"), ("cos", "o"), ("Dat", "INS"),
        ("Dec", "F6"), ("Deg", "F2"), ("dms", "m"), ("Dword", "F2"), ("Exp", "x"), ("F-E", "v"), ("Grad", "F4"),
        ("Hex", "F5"), ("Hyp", "h"), ("ln", "n"), ("Int", ";"), ("Inv", "i"), ("log", "l"), ("Lsh", "<"),
        ("M+", "CTRL+P"), ("MC", "CTRL+L"), ("Mod", "%"), ("MR", "CTRL+R"), ("MS", "CTRL+M"), ("n!", "!"), ("Not", "~"),
        ("Oct", "F7"), ("Or", "|"), ("PI", "p"), ("Rad", "F3"), ("s", "CTRL+D"), ("sin", "s"), ("sqrt", "@"),
        ("Sta", "CTRL+S"), ("Sum", "CTRL+T"), ("tan", "t"), ("Word", "F3"), ("Xor", "^"), ("x^2", "@"), ("x^3", "#"),
        ("x^y", "y"),
    ]
}

const KEYS_95: &[KeyRow] = &win9x_keys("BACKSP");
const KEYS_98: &[KeyRow] = &win9x_keys("Backspace");

#[rustfmt::skip]
const KEYS_2000: &[KeyRow] = &[
    ("%", "%"), ("(", "("), (")", ")"), ("*", "*"), ("+", "+"), ("+/-", "F9"), ("-", "-"), (".", ". or ,"),
    ("/", "/"), ("0-9", "0-9"), ("1/x", "r"), ("=", "ENTER"), ("A-F", "A-F"), ("And", "&"), ("Ave", "CTRL+A"),
    ("Backspace", "BACKSPACE"), ("Bin", "F8"), ("Byte", "F4"), ("C", "ESC"), ("CE", "DEL"), ("cos", "o"),
    ("Dat", "INS"), ("Dec", "F6"), ("Degrees", "F2"), ("dms", "m"), ("Dword", "F2"), ("Exp", "x"), ("F-E", "v"),
    ("Grads", "F4"), ("Hex", "F5"), ("Hyp", "h"), ("Int", ";"), ("Inv", "i"), ("ln", "n"), ("log", "l"), ("Lsh", "<"),
    ("M+", "CTRL+P"), ("MC", "CTRL+L"), ("Mod", "%"), ("MR", "CTRL+R"), ("MS", "CTRL+M"), ("n!", "!"), ("Not", "~"),
    ("Oct", "F7"), ("Or", "|(pipe)"), ("pi", "p"), ("Qword", "F12"), ("Radians", "F3"), ("s", "CTRL+D"), ("sin", "s"),
    ("sqrt", "@"), ("Sta", "CTRL+S"), ("Sum", "CTRL+T"), ("tan", "t"), ("Word", "F3"), ("Xor", "^"), ("x^2", "@"),
    ("x^3", "#"), ("x^y", "y"),
];

/// The keyboard list of `era`'s book.
pub fn keyboard_list(era: Era) -> &'static [KeyRow] {
    match era {
        Era::Win95 => KEYS_95,
        Era::Win98 => KEYS_98,
        Era::Win2000 => KEYS_2000,
    }
}

/// The printed lists read down the columns. Both WinHelp and HTML Help show the entries by row.
fn keyboard_table(era: Era) -> String {
    let keys = keyboard_list(era);
    let columns = if era == Era::Win2000 { 2 } else { 4 };
    let height = keys.len().div_ceil(columns);
    let heading = match era {
        Era::Win95 => "Button =\tKey",
        Era::Win98 => "Button = Key",
        Era::Win2000 => "Button\tKey",
    };
    let mut rows = vec![vec![heading; columns].join("\t")];
    let separator = if era == Era::Win98 { " " } else { "\t" };
    for i in 0..height {
        let row = keys
            .iter()
            .skip(i)
            .step_by(height)
            .map(|(button, key)| format!("{button}{separator}{key}"))
            .collect::<Vec<_>>();
        rows.push(row.join("\t"));
    }
    rows.join("\n")
}

fn page(label: &str, depth: usize, topic: usize, icon: EntryIcon) -> Entry {
    Entry { label: label.into(), depth, topic: Some(topic), icon }
}

fn book(label: &str, depth: usize) -> Entry {
    Entry { label: label.into(), depth, topic: None, icon: EntryIcon::Book }
}

/// Every keyword with each topic it opens, in the original's order of topics.
fn keywords(rows: &[(&str, &[usize])]) -> Vec<Keyword> {
    rows.iter()
        .flat_map(|&(label, topics)| topics.iter().map(move |&topic| Keyword { label: label.into(), topic }))
        .collect()
}

pub fn catalog(era: Era) -> Catalog {
    match era {
        Era::Win95 => windows_95(),
        Era::Win98 => windows_98(),
        Era::Win2000 => windows_2000(),
    }
}

/// CALC.HLP's nine topics with a title, in its order; the Contents of CALC.CNT.
fn windows_95() -> Catalog {
    const SIMPLE: usize = 0;
    const MEMORY: usize = 1;
    const STATS: usize = 2;
    const SCI: usize = 3;
    const CONVERT: usize = 4;
    const KEYS: usize = 5;
    const KEYLIST: usize = 6;
    const SEQ: usize = 7;
    const BUTTONS: usize = 8;
    let keys = keyboard_table(Era::Win95);
    let topics = vec![
        Topic::from_paragraphs(
            "Performing a simple calculation",
            &[
                (Title, "To perform a simple calculation"),
                (Step(1), "Enter the first number in the calculation."),
                (Step(2), "Click + to add, - to subtract, * to multiply, or / to divide."),
                (Step(3), "Enter the next number in the calculation."),
                (Step(4), "Enter any remaining operators and numbers."),
                (Step(5), "Click =."),
                (Subhead, "Tips"),
                (Bullet, "To use your numeric keypad to enter numbers and operators, press NUM LOCK."),
                (
                    Bullet,
                    "For Help on a Calculator button or setting, use the right mouse button to click it, and then click the What's This? command.",
                ),
            ],
        ),
        Topic::from_paragraphs(
            "Working with numbers stored in memory",
            &[
                (Title, "Working with numbers stored in memory"),
                (
                    Bullet,
                    "Click MS to store a number. When you store a number in memory, an M appears in the box above the memory options.",
                ),
                (Bullet, "To recall a displayed number, click MR."),
                (Bullet, "If you store another number in memory, it replaces the number currently in memory."),
                (Bullet, "To clear the memory, click MC."),
                (Bullet, "To add the displayed number to the number already in memory, click M+."),
            ],
        ),
        Topic::from_paragraphs(
            "Performing a statistical calculation",
            &[
                (Title, "To perform a statistical calculation"),
                (Step(1), "On the View menu, click Scientific."),
                (Step(2), "Enter your first piece of data."),
                (Step(3), "Click Sta, and then click Dat."),
                (Step(4), "Enter the rest of the data, clicking Dat after each entry."),
                (Step(5), "Click Sta."),
                (Step(6), "Click the button for the statistics function you want to use."),
                (Subhead, "Tips"),
                (
                    Bullet,
                    "To display the number selected in the Statistics Box in the Calculator display area, click LOAD.",
                ),
                (Bullet, "To use your numeric keypad to enter numbers and operators, press NUM LOCK."),
                (
                    Bullet,
                    "For Help on a function button or setting, use your right mouse button to click it, and then click the What's This? command.",
                ),
            ],
        ),
        Topic::from_paragraphs(
            "Performing a scientific calculation",
            &[
                (Title, "To perform a scientific calculation"),
                (Step(1), "On the View menu, click Scientific."),
                (Step(2), "Click a number system."),
                (Step(3), "Enter the first number."),
                (Step(4), "Click an operator."),
                (Step(5), "Enter the next number in the calculation."),
                (Step(6), "Enter any remaining operators and numbers."),
                (Step(7), "Click =."),
                (Subhead, "Tips"),
                (Bullet, "To use your numeric keypad to enter numbers and operators, press NUM LOCK."),
                (
                    Bullet,
                    "For Help on a function button or setting, use your right mouse button to click it, and then click the What's This? command.",
                ),
            ],
        ),
        Topic::from_paragraphs(
            "Converting numbers to other number systems",
            &[
                (Title, "To convert a value to another number system"),
                (Step(1), "On the View menu, click Scientific."),
                (Step(2), "Enter the number."),
                (Step(3), "Click the number system you want to convert to."),
                (Step(4), "Click the unit of measurement in which to display the result."),
                (Subhead, "Tips"),
                (
                    Bullet,
                    "For help on a number system or unit of measurement, use your right mouse button to click it, and then click the What's This? command.",
                ),
                (
                    Bullet,
                    "If you convert a decimal number containing decimal places to another number system, Calculator shortens the number to its integer.",
                ),
                (Bullet, "Numbers converted from hexadecimal, octal, or binary to decimal appear as integers."),
            ],
        ),
        Topic::from_paragraphs(
            "Finding out a keyboard equivalent",
            &[
                (Title, "To find out keyboard equivalents"),
                (
                    Bullet,
                    "Use your right mouse button to click any Calculator button, and then click the What's This? command.",
                ),
                (Subhead, "Tip"),
                (
                    Bullet,
                    "To see a complete list of keyboard equivalents and keyboard sequences, click Related Topics, below.",
                ),
                (Related(vec![KEYLIST, SEQ]), "Related Topics"),
            ],
        ),
        Topic::from_paragraphs(
            "List of keyboard equivalents",
            &[
                (Title, "Keyboard equivalents"),
                (Bullet, "To print this alphabetical list, click Options, and then click Print Topic."),
                (Table { header: true }, &keys),
            ],
        ),
        Topic::from_paragraphs(
            "List of key sequences",
            &[
                (Title, "Keyboard sequences"),
                (Body, "The following key sequences are interpreted as functions when you paste data into Calculator:"),
                (
                    Table { header: false },
                    concat!(
                        ":c\tClears memory.\n",
                        ":e\tEnables you to enter scientific notation numbers in decimal form. Also specifies the number E in hexadecimal.\n",
                        ":m\tStores the displayed number in memory.\n",
                        ":p\tAdds the displayed number to the number in memory.\n",
                        ":q\tClears the current calculation.\n",
                        ":r\tDisplays the number stored in memory.\n",
                        "\\\tFunctions the same as the Dat button. Click Sta before using this key.",
                    ),
                ),
            ],
        ),
        Topic::from_paragraphs(
            "To find out what a Calculator button does",
            &[
                (Title, "To find out what a Calculator button does"),
                (Step(1), "Using your right mouse button, click a Calculator button or control."),
                (Step(2), "Click the What's This? command."),
            ],
        ),
    ];
    let b = BUTTONS;
    #[rustfmt::skip]
    let index = keywords(&[
        ("- button", &[b]), ("% button", &[b]), ("* button", &[b]), ("+ button", &[b]), ("= button", &[b]),
        ("1/x button", &[b]), ("A button", &[b]),
        ("adding numbers to a value stored in memory", &[MEMORY]), ("addition", &[SIMPLE]),
        ("And button", &[b]), ("Ave button", &[b]), ("B button", &[b]), ("Back button", &[b]), ("Bin button", &[b]),
        ("binary number system", &[SCI, CONVERT]), ("buttons", &[KEYLIST, b, KEYS]),
        ("buttons, finding out what a Calculator button does", &[b]),
        ("buttons, keyboard equivalents for", &[KEYLIST, KEYS]), ("C button", &[b]), ("CAD button", &[b]),
        ("calculations", &[KEYS, SCI, KEYLIST, b, MEMORY, SIMPLE, STATS]),
        ("calculations, entering by using the keyboard", &[KEYLIST, KEYS]),
        ("calculations, entering numbers and operators", &[SCI, STATS, SIMPLE]),
        ("calculations, scientific", &[SCI]), ("calculations, statistical", &[STATS]),
        ("calculations, tips", &[b, MEMORY, KEYS]), ("CD button", &[b]), ("CE button", &[b]),
        ("clearing values stored in memory", &[MEMORY]),
        ("converting values to another number system", &[CONVERT]), ("cos button", &[b]),
        ("cosine", &[SCI]), ("cube root", &[SCI]), ("cubing", &[SCI]), ("D button", &[b]),
        ("Dat button", &[b, STATS]), ("Dec button", &[b]), ("decimal number system", &[CONVERT, SCI]),
        ("definitions of Calculator buttons", &[b]), ("Deg button", &[b]),
        ("deleting values stored in memory", &[MEMORY]), ("division", &[SIMPLE]), ("dms button", &[b]),
        ("E button", &[b]), ("entering numbers and operators", &[SIMPLE, STATS, SCI]),
        ("erasing values stored in memory", &[MEMORY]), ("Exp button", &[b]),
        ("explanations of Calculator buttons", &[b]), ("exponent", &[SCI]), ("F button", &[b]),
        ("factorials", &[SCI]), ("F-E button", &[b]), ("functions, key sequences for", &[SEQ]),
        ("Grad button", &[b]), ("Help on Calculator buttons, displaying", &[b]), ("Hex button", &[b]),
        ("hexadecimal number system", &[CONVERT, SCI]),
        // The original's own spelling of these two keywords.
        ("Hyp functin", &[b]), ("hyperbolic functions", &[SCI]), ("In button", &[b]),
        ("Int button", &[b]), ("Inv function", &[b]), ("inverting functions", &[SCI]),
        ("key sequences for functions", &[SEQ]), ("keyboard equivalents for Calculator buttons", &[KEYS, KEYLIST]),
        ("LOAD button", &[b, STATS]), ("log button", &[b]), ("logarithms", &[SCI]), ("Lsh button", &[b]),
        ("M+ button", &[MEMORY, b]), ("MC button", &[MEMORY, b]),
        ("memory, working with numbers stored in", &[MEMORY]), ("Mod button", &[b]), ("modulus", &[SCI]),
        ("MR button", &[MEMORY, b]), ("MS button", &[b, MEMORY]), ("multiplication", &[SIMPLE]),
        ("n! button", &[b]), ("Not button", &[b]), ("NUM LOCK key", &[STATS, SIMPLE, SCI]),
        ("number systems", &[SCI, CONVERT]), ("numbers", &[SIMPLE, SCI, CONVERT, STATS, MEMORY]),
        ("numbers, converting to another number system", &[CONVERT]),
        ("numbers, entering", &[SIMPLE, SCI, STATS]), ("numbers, storing in memory", &[MEMORY]),
        ("numeric keypad, using", &[SIMPLE, STATS, SCI]), ("Oct button", &[b]),
        ("octal number system", &[SCI, CONVERT]), ("operators, entering", &[SCI, SIMPLE]), ("Or button", &[b]),
        ("pasting data into Calculator", &[SEQ]), ("PI button", &[b]), ("printing keyboard equivalents", &[KEYLIST]),
        ("Rad button", &[b]), ("recalling a stored number", &[MEMORY]),
        ("removing values stored in memory", &[MEMORY]), ("RET button", &[b]), ("s button", &[b]),
        ("scientific calculations", &[SCI]), ("scientific notation", &[SCI]), ("sin button", &[b]),
        ("sine", &[SCI]), ("sqrt button", &[b]), ("square root", &[SCI]), ("squaring", &[SCI]),
        ("Sta button", &[b, STATS]), ("statistical calculations", &[STATS]), ("statistics box", &[STATS]),
        ("storing a number in memory", &[MEMORY]), ("subtraction", &[SIMPLE]), ("Sum button", &[b]),
        ("tan button", &[b]), ("tangent", &[SCI]), ("tips", &[MEMORY, b, KEYS]), ("x^2 button", &[b]),
        ("x^3 button", &[b]), ("x^y button", &[b]), ("Xor button", &[b]),
    ]);
    Catalog {
        title: "Calculator Help".into(),
        home: SIMPLE,
        topics,
        contents: vec![
            book("Performing Calculations", 0),
            page("Performing a simple calculation", 1, SIMPLE, EntryIcon::Page),
            page("Performing a statistical calculation", 1, STATS, EntryIcon::Page),
            page("Performing a scientific calculation", 1, SCI, EntryIcon::Page),
            page("Converting values to other number systems", 1, CONVERT, EntryIcon::Page),
            book("Tips and Tricks", 0),
            page("Finding out what a Calculator button does", 1, BUTTONS, EntryIcon::Page),
            page("Working with numbers stored in memory", 1, MEMORY, EntryIcon::Page),
            page("Using keyboard equivalents", 1, KEYS, EntryIcon::Page),
        ],
        index,
    }
}

/// calc.chm of Windows 98: its ten topics in the order of its Contents, the welcome page and the
/// legal page behind the welcome page's link.
fn windows_98() -> Catalog {
    const SIMPLE: usize = 0;
    const STATS: usize = 1;
    const SCI: usize = 2;
    const CONVERT: usize = 3;
    const BUTTONS: usize = 4;
    const MEMORY: usize = 5;
    const KEYLIST: usize = 6;
    const SEQ: usize = 7;
    const PRECISION: usize = 8;
    const TRANSFER: usize = 9;
    const WELCOME: usize = 10;
    const LEGAL: usize = 11;
    let keys = keyboard_table(Era::Win98);
    let mut topics = vec![
        Topic::from_paragraphs(
            "To perform a simple calculation",
            &[
                (Subhead, "To perform a simple calculation"),
                (Step(1), "Enter the first number in the calculation."),
                (Step(2), "Click + to add, - to subtract, * to multiply, or / to divide."),
                (Step(3), "Enter the next number in the calculation."),
                (Step(4), "Enter any remaining operators and numbers."),
                (Step(5), "Click =."),
                (Note, "Note"),
                (Bullet, "You can also use your numeric keypad to enter numbers and operators by pressing NUM LOCK."),
            ],
        ),
        Topic::from_paragraphs(
            "To perform a statistical calculation",
            &[
                (Subhead, "To perform a statistical calculation"),
                (Step(1), "On the View menu, click Scientific."),
                (Step(2), "Enter your first piece of data."),
                (Step(3), "Click Sta, and then click Dat."),
                (Step(4), "Enter the rest of the data, clicking Dat after each entry."),
                (Step(5), "Click Sta."),
                (Step(6), "Click the button for the statistics function you want to use."),
                (Note, "Note"),
                (Bullet, "You can also use your numeric keypad to enter numbers and operators by pressing NUM LOCK."),
            ],
        ),
        Topic::from_paragraphs(
            "To perform a scientific calculation",
            &[
                (Subhead, "To perform a scientific calculation"),
                (Step(1), "On the View menu, click Scientific."),
                (Step(2), "Click a number system."),
                (Step(3), "Enter the first number."),
                (Step(4), "Click an operator."),
                (Step(5), "Enter the next number in the calculation."),
                (Step(6), "Enter any remaining operators and numbers."),
                (Step(7), "Click =."),
                (Note, "Note"),
                (Bullet, "You can also use your numeric keypad to enter numbers and operators by pressing NUM LOCK."),
            ],
        ),
        Topic::from_paragraphs(
            "To convert a value to another number system",
            &[
                (Subhead, "To convert a value to another number system"),
                (Step(1), "On the View menu, click Scientific."),
                (Step(2), "Enter the number you want to convert."),
                (Step(3), "Click the number system you want to convert to."),
                (Note, "Notes"),
                (
                    Bullet,
                    "When you convert a decimal number that contains decimal places to another number system, the number is shortened to an integer.",
                ),
                (Bullet, "Numbers converted from hexadecimal, octal, or binary to decimal appear as integers."),
            ],
        ),
        Topic::from_paragraphs(
            "To see what a Calculator button does",
            &[
                (Subhead, "To see what a Calculator button does"),
                (Body, "Right-click a Calculator button, and then click What’s This?."),
            ],
        ),
        Topic::from_paragraphs(
            "To work with numbers stored in memory",
            &[
                (Subhead, "To work with numbers stored in memory"),
                (Bullet, "To store the displayed number, click MS."),
                (
                    Continuation,
                    "When you store a number, an M appears in the box above the memory options. If you store another number, it replaces the one currently in memory.",
                ),
                (Bullet, "To recall a stored number, click MR."),
                (Bullet, "To clear the memory, click MC."),
                (
                    Bullet,
                    "To add the displayed number to the number already in memory, click M+. To see the new number, click MR.",
                ),
            ],
        ),
        Topic::from_paragraphs(
            "Using keyboard equivalents of Calculator buttons",
            &[
                (Subhead, "Using keyboard equivalents of Calculator buttons"),
                (
                    Body,
                    "The following is an alphabetical list of Calculator buttons and their keyboard equivalents. You can print this list by clicking Options and then clicking Print. To see any Calculator button's keyboard equivalent, right-click the button, and then click What's This?.",
                ),
                (Table { header: true }, &keys),
            ],
        ),
        Topic::from_paragraphs(
            "Using key sequences as functions",
            &[
                (Subhead, "Using key sequences as functions"),
                (Body, "The following key sequences are interpreted as functions when you paste data into Calculator."),
                (Body, ":c Clears memory."),
                (
                    Body,
                    ":e Enables you to enter scientific notation numbers in decimal form. Also specifies the number E in hexadecimal.",
                ),
                (Body, ":m Stores the displayed number in memory."),
                (Body, ":p Adds the displayed number to the number in memory."),
                (Body, ":q Clears the current calculation."),
                (Body, ":r Displays the number stored in memory."),
                (Body, "\\ Functions the same as Dat. Click Sta before using this key."),
            ],
        ),
        Topic::from_paragraphs(
            "Understanding extended precision",
            &[
                (Subhead, "Understanding extended precision"),
                (
                    Body,
                    "Extended Precision, a feature of Calculator, implies that all operations are accurate to at least 32 digits. Calculator also stores rational numbers as fractions to retain accuracy. For example, 1/3 is stored as 1/3, rather than .333. However, errors accumulate during repeated operations on irrational numbers. For example, Calculator will truncate pi to 32 digits, so repeated operations on pi will lose accuracy as the number of operations increases.",
                ),
            ],
        ),
        Topic::from_paragraphs(
            "To transfer numbers between Standard and Scientific view",
            &[
                (Subhead, "To transfer numbers between Standard and Scientific view"),
                (Step(1), "Click MS to store the displayed number."),
                (Step(2), "On the View menu, click the desired view."),
                (Step(3), "Click MR to recall the stored number."),
                (Note, "Notes"),
                (
                    Bullet,
                    "Calculator clears the display when switching between Standard and Scientific view. Performing the above steps will allow numbers to be transferred between views.",
                ),
                (
                    Bullet,
                    "A number entered in Hexadecimal, Octal, or Binary format will be converted to Decimal format when transferring from Scientific to Standard view.",
                ),
            ],
        ),
        Topic::from_paragraphs(
            "Welcome to Help",
            &[
                (Title, "Welcome to Help"),
                (Body, "Use the Help system to learn more about Windows 98."),
                (Bullet, "Find answers to your questions."),
                (Bullet, "Browse the online version of the Getting Started book."),
                (Bullet, "Connect to the Web to get software updates."),
                (Bullet, "Troubleshoot your system."),
                (Link(LEGAL), "©1998 Microsoft Corporation. All rights reserved."),
            ],
        ),
        Topic::from_paragraphs(
            "Legal Information",
            &[
                (Title, "Legal Information"),
                (Body, "Microsoft Windows 98"),
                (
                    Body,
                    "Information in this document is subject to change without notice. The names of companies, products, people, characters, and/or data mentioned herein are fictitious and are in no way intended to represent any real individual, company, product, or event, unless otherwise noted. Complying with all applicable copyright laws is the responsibility of the user. No part of this document may be reproduced or transmitted in any form or by any means, electronic or mechanical, for any purpose, without the express written permission of Microsoft Corporation.",
                ),
                (
                    Body,
                    "Microsoft may have patents, patent applications, trademarks, copyrights, or other intellectual property rights covering subject matter in this document. Except as expressly provided in any written license agreement from Microsoft, the furnishing of this document does not give you any license to these patents, trademarks, copyrights, or other intellectual property.",
                ),
                (Body, "© 1998 Microsoft Corporation. All rights reserved."),
                (
                    Body,
                    "Microsoft, ActiveX, BackOffice, MS, MS-DOS, MSN, Windows, and Windows NT are either registered trademarks or trademarks of Microsoft Corporation in the U.S.A. and/or other countries.",
                ),
                (
                    Body,
                    "Other product and company names mentioned herein may be the trademarks of their respective owners.",
                ),
            ],
        ),
    ];
    topics[WELCOME].welcome = true;
    let subs = |parent: &str| -> Vec<(String, Vec<usize>)> {
        [
            ("adding", vec![SIMPLE, MEMORY, SEQ]),
            ("clearing", vec![MEMORY, SEQ]),
            ("converting", vec![CONVERT, TRANSFER]),
            ("dividing", vec![SIMPLE]),
            ("in memory", vec![TRANSFER, MEMORY, SEQ]),
            ("multiplying", vec![SIMPLE]),
            ("subtracting", vec![SIMPLE]),
            ("systems", vec![CONVERT]),
        ]
        .into_iter()
        .map(|(sub, topics)| (format!("{parent}, {sub}"), topics))
        .collect()
    };
    #[rustfmt::skip]
    let top: Vec<(&str, &[usize])> = vec![
        ("- (subtraction operator)", &[SIMPLE]), ("* (multiplication operator)", &[SIMPLE]),
        ("/ (division operator)", &[SIMPLE]), ("\\ (backslash)", &[SEQ]), ("+ (addition operator)", &[SIMPLE]),
        ("accuracy of Calculator", &[PRECISION]), ("adding", &[SIMPLE, MEMORY, SEQ]),
        ("asterisk (multiplication operator)", &[SIMPLE]), ("backslash", &[SEQ]),
        ("binary calculations", &[CONVERT, SCI]), ("buttons", &[BUTTONS, KEYLIST]),
        ("calculations", &[CONVERT, SCI, SIMPLE, STATS, MEMORY, PRECISION]),
        ("clearing values", &[TRANSFER, MEMORY, SEQ]), ("converting values", &[CONVERT, TRANSFER]),
        ("copying data", &[SEQ]), ("decimal calculations", &[CONVERT, SCI, SEQ]), ("display area", &[SEQ]),
        ("displaying numbers from memory", &[TRANSFER, MEMORY, SEQ]), ("dividing", &[SIMPLE]),
        ("entering numbers", &[SIMPLE]), ("equals sign", &[SIMPLE]), ("extended precision", &[PRECISION]),
        ("functions", &[SCI, BUTTONS, SEQ]), ("Help on buttons", &[BUTTONS]),
        ("hexadecimal calculations", &[CONVERT, SCI, SEQ]), ("irrational numbers, accuracy", &[PRECISION]),
        ("keyboard equivalents", &[SEQ, KEYLIST]), ("keyboard sequences", &[SEQ]),
        ("keypad", &[SCI, SIMPLE, STATS]), ("memory", &[TRANSFER, MEMORY, SEQ]), ("minus sign", &[SIMPLE]),
        ("multiplying", &[SIMPLE]), ("NUM LOCK key", &[SCI, SIMPLE, STATS]),
        ("numbers", &[MEMORY, SEQ, CONVERT, TRANSFER, SIMPLE]),
        ("numeric keypad", &[SCI, SIMPLE, STATS]), ("octal calculations", &[CONVERT, SCI]),
        ("operators", &[SCI, SIMPLE, STATS]), ("pasting data", &[SEQ]), ("plus sign", &[SIMPLE]),
        ("precision", &[PRECISION]), ("rational numbers", &[PRECISION]),
        ("recalling numbers from memory", &[TRANSFER, MEMORY, SEQ]),
        ("scientific calculations", &[CONVERT, SCI, STATS, TRANSFER, SEQ]),
        ("showing numbers in memory", &[TRANSFER, MEMORY, SEQ]), ("simple calculations", &[SIMPLE]),
        ("slash (division operator)", &[SIMPLE]), ("standard view", &[SIMPLE, TRANSFER]),
        ("statistical calculations", &[STATS, BUTTONS, SEQ]),
        ("storing numbers in memory", &[TRANSFER, MEMORY, SEQ]), ("subtracting", &[SIMPLE]),
        ("systems, number", &[CONVERT]), ("tips on buttons", &[BUTTONS]),
        ("transferring numbers between views", &[TRANSFER]),
        ("values", &[MEMORY, SEQ, CONVERT, TRANSFER, SIMPLE]),
        ("views", &[SCI, SIMPLE, STATS, TRANSFER]),
    ];
    let mut index = keywords(&top);
    for parent in ["numbers", "values"] {
        for (label, topics) in subs(parent) {
            index.extend(topics.into_iter().map(|topic| Keyword { label: label.clone(), topic }));
        }
    }
    Catalog {
        title: "Calculator Help".into(),
        home: WELCOME,
        topics,
        contents: vec![
            book("Performing Calculations", 0),
            page("Perform a simple calculation ", 1, SIMPLE, EntryIcon::Page),
            page("Perform a statistical calculation ", 1, STATS, EntryIcon::Page),
            page("Perform a scientific calculation ", 1, SCI, EntryIcon::Page),
            page("Convert values to other number systems ", 1, CONVERT, EntryIcon::Page),
            book("Tips and Tricks", 0),
            page("Find out what a Calculator button does", 1, BUTTONS, EntryIcon::Page),
            page("Work with numbers stored in memory", 1, MEMORY, EntryIcon::Page),
            page("Use keyboard equivalents of Calculator buttons", 1, KEYLIST, EntryIcon::Page),
            page("Use key sequences as functions", 1, SEQ, EntryIcon::Page),
            page("Understanding extended precision", 1, PRECISION, EntryIcon::Page),
            page("Transfer numbers between standard and scientific view", 1, TRANSFER, EntryIcon::Page),
        ],
        index,
    }
}

/// calc.chm of Windows 2000: the overview and its eleven topics under one book, each with the
/// original's Related Topics inside this book.
fn windows_2000() -> Catalog {
    const OVERVIEW: usize = 0;
    const SIMPLE: usize = 1;
    const SCI: usize = 2;
    const STATS: usize = 3;
    const TRANSFER: usize = 4;
    const MEMORY: usize = 5;
    const GROUPING: usize = 6;
    const CONVERT: usize = 7;
    const KEYLIST: usize = 8;
    const SEQ: usize = 9;
    const PRECISION: usize = 10;
    const OVERFLOW: usize = 11;
    let keys = keyboard_table(Era::Win2000);
    let mut topics = vec![
        Topic::from_paragraphs(
            "Calculator overview",
            &[
                (Title, "Calculator overview"),
                (
                    Body,
                    "You can use Calculator to perform any of the standard operations for which you would normally use a handheld calculator. Calculator performs basic arithmetic, such as addition and subtraction, as well as functions found on a scientific calculator, such as logarithms and factorials.",
                ),
            ],
        ),
        Topic::from_paragraphs(
            "Perform a simple calculation",
            &[
                (Subhead, "To perform a simple calculation"),
                (Step(1), "Type the first number in the calculation."),
                (Step(2), "Click + to add, - to subtract, * to multiply, or / to divide."),
                (Step(3), "Type the next number in the calculation."),
                (Step(4), "Type any remaining operators and numbers."),
                (Step(5), "Click =."),
                (Note, "Note"),
                (Bullet, "You can also use your numeric keypad to type numbers and operators by pressing NUM LOCK."),
            ],
        ),
        Topic::from_paragraphs(
            "Perform a scientific calculation",
            &[
                (Subhead, "To perform a scientific calculation"),
                (Step(1), "On the View menu, click Scientific."),
                (Step(2), "Click a number system."),
                (Step(3), "Click the display size you want to use, and then continue with your calculation."),
                (Note, "Note"),
                (Bullet, "You can also use your numeric keypad to type numbers and operators by pressing NUM LOCK."),
            ],
        ),
        Topic::from_paragraphs(
            "Perform a statistical calculation",
            &[
                (Subhead, "To perform a statistical calculation"),
                (Step(1), "On the View menu, click Scientific."),
                (Step(2), "Type your first piece of data, and then click Sta to open the Statistics Box."),
                (Step(3), "Click RET to return to Calculator, and then click Dat to save the value."),
                (Step(4), "Type the rest of the data, clicking Dat after each entry."),
                (Step(5), "Click Ave, Sum, or s."),
                (Note, "Notes"),
                (Bullet, "You can also use your numeric keypad to type numbers and operators by pressing NUM LOCK."),
                (
                    Bullet,
                    "Ave calculates the mean of the values saved in the Statistics Box, Sum calculates the sum of the values, and s calculates the standard deviation.",
                ),
                (Bullet, "After you have entered all of your data, you can see the list by clicking Sta."),
                (
                    Bullet,
                    "The Statistics Box tracks the number of values that you have saved at the bottom of the dialog box. You can delete a specific value from the list by clicking CD, or you can delete all of the values by clicking CAD. Clicking Load changes the number in the Calculator display area to the number selected in the Statistics Box.",
                ),
            ],
        ),
        Topic::from_paragraphs(
            "Transfer numbers between Standard and Scientific view",
            &[
                (Subhead, "To transfer numbers between Standard and Scientific view"),
                (Step(1), "Click MS to store the displayed number."),
                (Step(2), "On the View menu, click the desired view."),
                (Step(3), "Click MR to recall the stored number."),
                (Note, "Notes"),
                (
                    Bullet,
                    "Calculator clears the display when switching between the Standard and Scientific views. Steps 1-3 above allow numbers to be transferred between views.",
                ),
                (
                    Bullet,
                    "A number typed in hexadecimal, octal, or binary format will be converted to decimal format when transferring from Scientific to Standard view.",
                ),
            ],
        ),
        Topic::from_paragraphs(
            "Work with numbers stored in memory",
            &[
                (Subhead, "To work with numbers stored in memory"),
                (Bullet, "To store the displayed number, click MS."),
                (Bullet, "To recall a stored number, click MR."),
                (Bullet, "To clear the memory, click MC."),
                (
                    Bullet,
                    "To add the displayed number to the number already in memory, click M+. To see the new number, click MR.",
                ),
                (Note, "Note"),
                (
                    Bullet,
                    "When you store a number, an M appears in the box above the memory options. If you store another number, it replaces the one currently in memory.",
                ),
            ],
        ),
        Topic::from_paragraphs(
            "Use digit grouping",
            &[
                (Subhead, "To use digit grouping"),
                (Bullet, "On the View menu, click Digit grouping."),
                (Note, "Notes"),
                (Bullet, "You can use digit grouping to view numbers as logically grouped."),
                (Bullet, "Calculator uses commas to group decimal numbers, and spaces to group hexadecimal numbers."),
            ],
        ),
        Topic::from_paragraphs(
            "Convert a value to another number system",
            &[
                (Subhead, "To convert a value to another number system"),
                (Step(1), "On the View menu, click Scientific."),
                (Step(2), "Type the number you want to convert."),
                (Step(3), "Click the number system you want to convert to."),
                (Step(4), "Click the display size you want to use."),
                (Note, "Notes"),
                (
                    Bullet,
                    "When you convert a decimal number that contains decimal places to another number system, the number is shortened to an integer.",
                ),
                (
                    Bullet,
                    "Numbers converted to decimal from hexadecimal, octal, or binary appear as positive integers.",
                ),
            ],
        ),
        Topic::from_paragraphs(
            "Using keyboard equivalents of Calculator buttons",
            &[
                (Title, "Using keyboard equivalents of Calculator buttons"),
                (
                    Body,
                    "The following is an alphabetical list of Calculator buttons and their keyboard equivalents. You can print this list by clicking the Options menu, and then clicking Print. When using Calculator, you can see any Calculator button's keyboard equivalent by right-clicking the button, and then clicking What's This?.",
                ),
                (Table { header: true }, &keys),
            ],
        ),
        Topic::from_paragraphs(
            "Using key sequences as functions",
            &[
                (Title, "Using key sequences as functions"),
                (
                    Body,
                    "The following key sequences are interpreted as functions when you paste data into Calculator. For example, open Notepad and type the following:",
                ),
                (Body, "123 :m"),
                (
                    Body,
                    "Copy 123 :m from Notepad and paste it into Calculator. The number 123 is displayed and is also stored in Calculator's memory.",
                ),
                (
                    Table { header: false },
                    concat!(
                        ":c\tClears memory.\n",
                        ":e\tEnables you to type scientific notation numbers in decimal form.\n",
                        ":m\tStores the displayed number in memory.\n",
                        ":p\tAdds the displayed number to the number in memory.\n",
                        ":q\tClears the current calculation.\n",
                        ":r\tDisplays the number stored in memory.\n",
                        "\\\tFunctions the same as Dat. Click Sta before using this key.",
                    ),
                ),
                (Note, "Note"),
                (
                    Bullet,
                    "To open an accessory, click Start, point to Programs, point to Accessories, and then click the appropriate icon.",
                ),
            ],
        ),
        Topic::from_paragraphs(
            "Understanding extended precision",
            &[
                (Title, "Understanding extended precision"),
                (
                    Body,
                    "Extended Precision, a feature of Calculator, means that all operations are accurate to at least 32 digits. Calculator also stores rational numbers as fractions to retain accuracy. For example, 1/3 is stored as 1/3, rather than .333. However, errors accumulate during repeated operations on irrational numbers. For example, Calculator will truncate pi to 32 digits, so repeated operations on pi will lose accuracy as the number of operations increases.",
                ),
            ],
        ),
        Topic::from_paragraphs(
            "Performing calculations with large numbers",
            &[
                (Title, "Performing calculations with large numbers"),
                (
                    Body,
                    "In the hexadecimal, binary, and octal number systems, Calculator displays only the lower digits of an answer when the result has more digits than your display size allows. This behavior mimics the way calculations work in computers.",
                ),
                (
                    Body,
                    "For the hexadecimal number system, QWORD results can contain up to 16 digits, DWORD results can contain up to eight digits; Word results can contain up to four digits; and Byte results can contain up to two digits.",
                ),
                (
                    Body,
                    "For example, using the hexadecimal number system displayed as Word, the largest result you can generate is FFFF (equal to 65535 in decimal). If you double that number (FFFFx2), the answer is 1FFFE. This contains five digits, so Calculator will display only the lower four digits of the answer: FFFE.",
                ),
            ],
        ),
    ];
    for (t, links) in topics.iter_mut().zip([
        vec![SIMPLE, SCI],
        vec![GROUPING, SCI, STATS],
        vec![GROUPING, SIMPLE, STATS],
        vec![GROUPING, SCI, SIMPLE],
        vec![MEMORY, SIMPLE, SCI, OVERFLOW],
        vec![KEYLIST, SEQ, TRANSFER],
        vec![SIMPLE, SCI, STATS],
        vec![OVERFLOW, GROUPING],
        vec![MEMORY, OVERFLOW, SIMPLE, SCI, STATS],
        vec![KEYLIST, MEMORY, SIMPLE, SCI, STATS],
        vec![OVERFLOW, SIMPLE, STATS, SCI],
        vec![PRECISION, CONVERT, KEYLIST],
    ]) {
        t.paragraphs.push(Paragraph { style: Related(links), text: "Related Topics".into() });
    }
    #[rustfmt::skip]
    let index = keywords(&[
        ("- (subtraction operator)", &[SIMPLE]), ("* (multiplication operator)", &[SIMPLE]),
        ("/ (division operator)", &[SIMPLE]), ("\\ (backslash)", &[SEQ]), ("+ (addition operator)", &[SIMPLE]),
        ("about Calculator", &[OVERVIEW]), ("accuracy of Calculator", &[PRECISION]), ("adding", &[SIMPLE, SEQ, MEMORY]),
        ("asterisk (multiplication operator)", &[SIMPLE]), ("averages", &[STATS]), ("backslash", &[SEQ]),
        ("binary calculations", &[CONVERT, SCI, OVERFLOW]), ("buttons", &[SEQ, KEYLIST]),
        ("Byte results", &[CONVERT, OVERFLOW]),
        ("calculations, converting to another number system", &[CONVERT]),
        ("calculations, extended precision", &[PRECISION]), ("calculations, large-number", &[OVERFLOW]),
        ("calculations, scientific", &[SCI]), ("calculations, simple", &[SIMPLE]),
        ("calculations, statistical", &[STATS]), ("calculations, stored numbers", &[MEMORY]),
        ("clearing current calculation", &[SEQ]), ("clearing memory", &[SEQ, MEMORY]),
        ("converting values", &[CONVERT, TRANSFER]), ("copying data", &[SEQ]),
        ("decimal calculations", &[CONVERT, OVERFLOW, SEQ, GROUPING]), ("degrees", &[CONVERT]),
        ("digit grouping", &[GROUPING]), ("display size", &[OVERFLOW]), ("displayed results", &[OVERFLOW]),
        ("displaying numbers stored in memory", &[SEQ, MEMORY]), ("dividing", &[SIMPLE]),
        ("Dword results", &[CONVERT, OVERFLOW]), ("entering numbers", &[SIMPLE]), ("equals sign", &[SIMPLE]),
        ("equivalents, keyboard", &[SEQ, KEYLIST]), ("extended precision", &[PRECISION]), ("functions", &[SEQ]),
        ("gradients", &[CONVERT]), ("grouping numbers", &[GROUPING]),
        ("hexadecimal calculations", &[CONVERT, SCI, OVERFLOW, SEQ, GROUPING]), ("irrational numbers", &[PRECISION]),
        ("keyboard equivalents", &[SEQ, KEYLIST]), ("keyboard sequences", &[SEQ]), ("keypad", &[SCI, SIMPLE, STATS]),
        ("large-number calculations", &[OVERFLOW]), ("means", &[STATS]), ("memory, common tasks", &[MEMORY]),
        ("memory, key sequences", &[SEQ]), ("minus sign", &[SIMPLE]), ("multiplying", &[SIMPLE]),
        ("NUM LOCK key", &[SCI, SIMPLE, STATS]), ("number systems", &[OVERFLOW, CONVERT, SCI, TRANSFER]),
        ("numeric keypad", &[SCI, SIMPLE, STATS]), ("octal calculations", &[CONVERT, SCI, OVERFLOW, TRANSFER]),
        ("operators", &[SCI, SIMPLE, STATS]), ("overflow", &[OVERFLOW]), ("overview", &[OVERVIEW]),
        ("pasting data", &[SEQ]), ("pi", &[PRECISION]), ("plus sign", &[SIMPLE]), ("precision", &[PRECISION]),
        ("printing a list of button-to-keyboard equivalents", &[KEYLIST]), ("Qword results", &[OVERFLOW]),
        ("radians", &[CONVERT]), ("rational numbers", &[PRECISION]),
        ("recalling numbers from memory", &[TRANSFER, SEQ, MEMORY]),
        ("scientific calculations", &[CONVERT, SCI, STATS, TRANSFER, SEQ]), ("scientific notation", &[SCI]),
        ("separators in digit grouping", &[GROUPING]), ("sequences, keyboard", &[SEQ, KEYLIST]),
        ("simple calculations", &[SIMPLE]), ("slash (division operator)", &[SIMPLE]),
        ("standard deviation", &[STATS]), ("standard view", &[TRANSFER]), ("statistical calculations", &[STATS, SEQ]),
        ("storing numbers in memory", &[TRANSFER, SEQ, MEMORY]), ("subtracting", &[SIMPLE]), ("sum", &[STATS]),
        ("systems, number", &[CONVERT, SCI, OVERFLOW]),
        ("transferring numbers between Standard and Scientific views", &[TRANSFER]), ("truncations", &[PRECISION]),
        ("viewing button-to-keyboard equivalents", &[KEYLIST]), ("views, transferring between", &[TRANSFER]),
        ("Word results", &[CONVERT, OVERFLOW]),
    ]);
    Catalog {
        title: "Calculator".into(),
        home: OVERVIEW,
        topics,
        contents: vec![
            book("Calculator", 0),
            page("Calculator overview", 1, OVERVIEW, EntryIcon::Overview),
            page("Perform a simple calculation ", 1, SIMPLE, EntryIcon::Page),
            page("Perform a scientific calculation ", 1, SCI, EntryIcon::Page),
            page("Perform a statistical calculation ", 1, STATS, EntryIcon::Page),
            page("Transfer numbers between Standard and Scientific view", 1, TRANSFER, EntryIcon::Page),
            page("Work with numbers stored in memory", 1, MEMORY, EntryIcon::Page),
            page("Use digit grouping", 1, GROUPING, EntryIcon::Page),
            page("Convert a value to another number system", 1, CONVERT, EntryIcon::Page),
            page("Using keyboard equivalents of Calculator buttons", 1, KEYLIST, EntryIcon::Overview),
            page("Using key sequences as functions", 1, SEQ, EntryIcon::Overview),
            page("Understanding extended precision", 1, PRECISION, EntryIcon::Overview),
            page("Performing calculations with large numbers", 1, OVERFLOW, EntryIcon::Overview),
        ],
        index,
    }
}

#[cfg(test)]
mod tests;
