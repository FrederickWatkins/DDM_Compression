//! Custom color-eyre reporting for SNAFU errors.

use std::{
    error::Error,
    fmt,
    panic::Location,
};

use color_eyre::{
    config::HookBuilder,
    eyre::{self, EyreHandler},
};

// -----------------------------------------------------------------------------
// SNAFU backtrace extraction
// -----------------------------------------------------------------------------

#[cfg(debug_assertions)]
type SnafuBacktraceExtractor =
    fn(&(dyn Error + 'static)) -> Option<snafu::Backtrace>;

#[cfg(debug_assertions)]
fn extract_snafu_backtrace<E>(
    error: &(dyn Error + 'static),
) -> Option<snafu::Backtrace>
where
    E: Error + snafu::ErrorCompat + 'static,
{
    let error = error.downcast_ref::<E>()?;
    snafu::ErrorCompat::backtrace(error).cloned()
}

/// Register every concrete SNAFU error type whose backtrace should be shown.
#[cfg(debug_assertions)]
const SNAFU_BACKTRACE_EXTRACTORS: &[SnafuBacktraceExtractor] = &[
    extract_snafu_backtrace::<ddm_core::coding::wavelet::Error>,
    extract_snafu_backtrace::<ddm_core::coding::cabac::Error>,
    extract_snafu_backtrace::<ddm_core::l0b::Error>,

    // Add the other module error types here:
    // extract_snafu_backtrace::<ddm_core::coding::cabac::Error>,
    // extract_snafu_backtrace::<ddm_core::l0b::mp00::Error>,
    // extract_snafu_backtrace::<ddm_core::l0b::mq00::Error>,
];

#[cfg(debug_assertions)]
fn find_snafu_backtrace(
    error: &(dyn Error + 'static),
) -> Option<snafu::Backtrace> {
    let mut current: Option<&(dyn Error + 'static)> = Some(error);

    while let Some(candidate) = current {
        if let Some(backtrace) = SNAFU_BACKTRACE_EXTRACTORS
            .iter()
            .find_map(|extract| extract(candidate))
        {
            return Some(backtrace);
        }

        current = candidate.source();
    }

    None
}

// -----------------------------------------------------------------------------
// Frame filtering
// -----------------------------------------------------------------------------

/// Identifies trait-qualified symbols such as `<T as Trait>::method`.
#[cfg(debug_assertions)]
fn is_unhelpful_angle_qualified_symbol(name: &str) -> bool {
    if !name.starts_with('<') {
        return false;
    }

    let mut depth = 0_usize;

    for (index, character) in name.char_indices() {
        match character {
            '<' => depth += 1,

            '>' if depth > 0 => {
                depth -= 1;

                if depth == 0 {
                    let suffix = &name[index + 1..];

                    return suffix.is_empty() || suffix.starts_with("::");
                }
            }

            _ => {}
        }
    }

    false
}

#[cfg(debug_assertions)]
fn is_unhelpful_frame(name: &str) -> bool {
    name == "main"
        || name.starts_with("main::")
        || name.starts_with("__libc_start")
        || name == "_start"
        || name.starts_with("_start::")
        || name.contains("::rt::lang_start")
        || name.contains("__rust_begin_short_backtrace")
        || is_unhelpful_angle_qualified_symbol(name)
}

// -----------------------------------------------------------------------------
// ANSI-aware symbol cleanup
// -----------------------------------------------------------------------------

/// A visible character or an ANSI CSI sequence in formatted output.
#[cfg(debug_assertions)]
struct SymbolToken<'a> {
    text: &'a str,
    character: Option<char>,
    start: usize,
}

/// Splits text into visible characters and ANSI CSI sequences.
///
/// ANSI sequences are treated as zero-width tokens, allowing symbol cleanup
/// to preserve colours without counting escape-code characters as brackets.
#[cfg(debug_assertions)]
fn tokenize_ansi(input: &str) -> Vec<SymbolToken<'_>> {
    let bytes = input.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == 0x1b
            && bytes.get(index + 1) == Some(&b'[')
        {
            let mut end = index + 2;

            while end < bytes.len()
                && !(0x40..=0x7e).contains(&bytes[end])
            {
                end += 1;
            }

            if end < bytes.len() {
                tokens.push(SymbolToken {
                    text: &input[index..=end],
                    character: None,
                    start: index,
                });

                index = end + 1;
                continue;
            }
        }

        let Some(character) = input[index..].chars().next() else {
            break;
        };

        let end = index + character.len_utf8();

        tokens.push(SymbolToken {
            text: &input[index..end],
            character: Some(character),
            start: index,
        });

        index = end;
    }

    tokens
}

#[cfg(debug_assertions)]
fn next_visible_token(
    tokens: &[SymbolToken<'_>],
    mut index: usize,
) -> Option<usize> {
    while index < tokens.len() {
        if tokens[index].character.is_some() {
            return Some(index);
        }

        index += 1;
    }

    None
}

/// Removes bracketed details from a single function-symbol string.
///
/// Examples:
///
/// - `crate[0123456789abcdef]::module::function`
///   becomes `crate::module::function`.
/// - `module::lift::<module::highpass_53>`
///   becomes `module::lift`.
///
/// ANSI colour sequences are preserved even when they occur inside text
/// being removed, so that later text is still coloured correctly.
#[cfg(debug_assertions)]
fn strip_symbol_details(symbol: &str) -> String {
    let tokens = tokenize_ansi(symbol);

    // Locate the two visible colons in `::<T>`, ignoring ANSI sequences
    // between characters.
    let mut turbofish_colons = vec![false; tokens.len()];

    for (index, token) in tokens.iter().enumerate() {
        if token.character != Some(':') {
            continue;
        }

        let Some(second) = next_visible_token(&tokens, index + 1) else {
            continue;
        };

        let Some(third) = next_visible_token(&tokens, second + 1) else {
            continue;
        };

        if tokens[second].character == Some(':')
            && tokens[third].character == Some('<')
        {
            turbofish_colons[index] = true;
            turbofish_colons[second] = true;
        }
    }

    let mut output = String::with_capacity(symbol.len());
    let mut angle_depth = 0_usize;
    let mut square_depth = 0_usize;

    for (index, token) in tokens.iter().enumerate() {
        let Some(character) = token.character else {
            // Always preserve colour changes, including inside omitted text.
            output.push_str(token.text);
            continue;
        };

        if angle_depth > 0 {
            match character {
                '<' => angle_depth += 1,
                '>' => angle_depth -= 1,
                _ => {}
            }

            continue;
        }

        if square_depth > 0 {
            match character {
                '[' => square_depth += 1,
                ']' => square_depth -= 1,
                _ => {}
            }

            continue;
        }

        if turbofish_colons[index] {
            continue;
        }

        match character {
            '<' => angle_depth = 1,
            '[' => square_depth = 1,

            // Discard stray closing delimiters rather than leaving broken
            // generic syntax if an upstream symbol is malformed.
            '>' | ']' => {}

            _ => output.push_str(token.text),
        }
    }

    output
}

/// Finds the first character of a function name in a numbered frame line.
///
/// ANSI codes are ignored when parsing the frame number. The returned byte
/// offset still refers to the original string, including its colour codes.
#[cfg(debug_assertions)]
fn frame_symbol_start(line: &str) -> Option<usize> {
    let tokens = tokenize_ansi(line);

    let visible: Vec<(usize, char)> = tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| {
            token.character.map(|character| (index, character))
        })
        .collect();

    let mut cursor = 0;

    while visible
        .get(cursor)
        .is_some_and(|(_, character)| character.is_whitespace())
    {
        cursor += 1;
    }

    let first_digit = cursor;

    while visible
        .get(cursor)
        .is_some_and(|(_, character)| character.is_ascii_digit())
    {
        cursor += 1;
    }

    if cursor == first_digit
        || visible.get(cursor).map(|(_, character)| *character) != Some(':')
    {
        return None;
    }

    cursor += 1;

    while visible
        .get(cursor)
        .is_some_and(|(_, character)| character.is_whitespace())
    {
        cursor += 1;
    }

    let (token_index, _) = visible.get(cursor)?;
    Some(tokens[*token_index].start)
}

/// Cleans only numbered function-symbol lines.
///
/// Crucially, source locations, source snippets, and other formatter output
/// are copied unchanged. This prevents `<unknown source file>` from being
/// mistaken for generic arguments.
#[cfg(debug_assertions)]
fn clean_backtrace_symbols(input: &str) -> String {
    let mut output = String::with_capacity(input.len());

    for line in input.split_inclusive('\n') {
        if let Some(symbol_start) = frame_symbol_start(line) {
            output.push_str(&line[..symbol_start]);
            output.push_str(&strip_symbol_details(&line[symbol_start..]));
        } else {
            output.push_str(line);
        }
    }

    output
}

// -----------------------------------------------------------------------------
// Custom EyreHandler
// -----------------------------------------------------------------------------

struct SnafuBacktraceHandler {
    inner: Box<dyn EyreHandler>,

    #[cfg(debug_assertions)]
    backtrace: Option<snafu::Backtrace>,
}

impl EyreHandler for SnafuBacktraceHandler {
    fn debug(
        &self,
        error: &(dyn Error + 'static),
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        // Preserve color-eyre's alternate Debug representation.
        if f.alternate() {
            return self.inner.debug(error, f);
        }

        // Render the normal color-eyre report first.
        self.inner.debug(error, f)?;

        #[cfg(debug_assertions)]
        if let Some(backtrace) = &self.backtrace {
            let printer = color_backtrace::BacktracePrinter::new()
                .strip_function_hash(true)
                .add_frame_filter(Box::new(|frames| {
                    frames.retain(|frame| {
                        let name = frame.name.as_deref().unwrap_or_default();

                        // These frames either add no useful diagnostic
                        // information or cannot provide source locations.
                        !is_unhelpful_frame(name)
                            && frame.filename.is_some()
                    });
                }));

            let formatted = printer
                .format_trace_to_string(backtrace)
                .unwrap_or_else(|_| format!("{backtrace:?}"));

            let formatted = clean_backtrace_symbols(&formatted);

            write!(f, "\n\nSNAFU backtrace:\n{formatted}")?;
        }

        Ok(())
    }

    fn display(
        &self,
        error: &(dyn Error + 'static),
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        self.inner.display(error, f)
    }

    fn track_caller(&mut self, location: &'static Location<'static>) {
        self.inner.track_caller(location);
    }
}

// -----------------------------------------------------------------------------
// Hook installation
// -----------------------------------------------------------------------------

/// Installs color-eyre reporting with SNAFU backtrace support.
///
/// Call once, near the start of `main`, before constructing any eyre reports.
pub fn install_error_reporting() -> Result<(), eyre::InstallError> {
    let (panic_hook, eyre_hook) = HookBuilder::default().into_hooks();

    let default_eyre_hook = eyre_hook.into_eyre_hook();

    eyre::set_hook(Box::new(move |error| {
        let inner = default_eyre_hook(error);

        Box::new(SnafuBacktraceHandler {
            inner,

            #[cfg(debug_assertions)]
            backtrace: find_snafu_backtrace(error),
        })
    }))?;

    panic_hook.install();

    Ok(())
}
