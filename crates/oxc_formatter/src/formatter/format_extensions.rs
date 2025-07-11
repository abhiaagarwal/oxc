use std::cell::OnceCell;

use super::{Buffer, prelude::*};

/// Utility trait that allows memorizing the output of a [Format].
/// Useful to avoid re-formatting the same object twice.
pub trait MemoizeFormat<'a> {
    /// Returns a formattable object that memoizes the result of `Format` by cloning.
    /// Mainly useful if the same sub-tree can appear twice in the formatted output because it's
    /// used inside of `if_group_breaks` or `if_group_fits_single_line`.
    ///
    /// ```
    /// use std::cell::Cell;
    /// use biome_formatter::{format, write};
    /// use biome_formatter::prelude::*;
    /// use biome_rowan::TextSize;
    ///
    /// struct MyFormat {
    ///   value: Cell<u64>
    /// }
    ///
    /// impl MyFormat {
    ///     pub fn new() -> Self {
    ///         Self { value: Cell::new(1) }
    ///     }
    /// }
    ///
    /// impl Format<SimpleFormatContext> for MyFormat {
    ///     fn fmt(&self, f: &mut Formatter<SimpleFormatContext>) -> FormatResult<()> {
    ///         let value = self.value.get();
    ///         self.value.set(value + 1);
    ///
    ///         write!(f, [dynamic_text(&std::format!("Formatted {value} times."), TextSize::from(0))])
    ///     }
    /// }
    ///
    /// # fn main() -> FormatResult<()> {
    /// let normal = MyFormat::new();
    ///
    /// // Calls `format` for everytime the object gets formatted
    /// assert_eq!(
    ///     "Formatted 1 times. Formatted 2 times.",
    ///     format!(SimpleFormatContext::default(), [normal, space(), normal])?.print()?.as_code()
    /// );
    ///
    /// // Memoized memoizes the result and calls `format` only once.
    /// let memoized = normal.memoized();
    /// assert_eq!(
    ///     "Formatted 3 times. Formatted 3 times.",
    ///     format![SimpleFormatContext::default(), [memoized, space(), memoized]]?.print()?.as_code()
    /// );
    /// # Ok(())
    /// # }
    /// ```
    ///
    fn memoized(self) -> Memoized<'a, Self>
    where
        Self: Sized + Format<'a>,
    {
        Memoized::new(self)
    }
}

impl<T> MemoizeFormat<'_> for T {}

/// Memoizes the output of its inner [Format] to avoid re-formatting a potential expensive object.
#[derive(Debug)]
pub struct Memoized<'ast, F> {
    inner: F,
    memory: OnceCell<FormatResult<Option<FormatElement<'ast>>>>,
}

impl<'ast, F> Memoized<'ast, F>
where
    F: Format<'ast>,
{
    fn new(inner: F) -> Self {
        Self { inner, memory: OnceCell::new() }
    }

    /// Gives access to the memoized content.
    ///
    /// Performs the formatting if the content hasn't been formatted at this point.
    ///
    /// # Example
    ///
    /// Inspect if some memoized content breaks.
    ///
    /// ```rust
    /// use std::cell::Cell;
    /// use biome_formatter::{format, write};
    /// use biome_formatter::prelude::*;
    /// use biome_rowan::TextSize;
    ///
    /// #[derive(Default)]
    /// struct Counter {
    ///   value: Cell<u64>
    /// }
    ///
    /// impl Format<SimpleFormatContext> for Counter {
    ///     fn fmt(&self, f: &mut Formatter<SimpleFormatContext>) -> FormatResult<()> {
    ///         let current = self.value.get();
    ///
    ///         write!(f, [
    ///             text("Count:"),
    ///             space(),
    ///             dynamic_text(&std::format!("{current}"), TextSize::default()),
    ///             hard_line_break()
    ///         ])?;
    ///
    ///         self.value.set(current + 1);
    ///         Ok(())
    ///     }
    /// }
    ///
    /// # fn main() -> FormatResult<()> {
    /// let content = format_with(|f| {
    ///     let mut counter = Counter::default().memoized();
    ///     let counter_content = counter.inspect(f)?;
    ///
    ///     if counter_content.will_break() {
    ///         write!(f, [text("Counter:"), block_indent(&counter)])
    ///     } else {
    ///         write!(f, [text("Counter:"), counter])
    ///     }?;
    ///
    ///     write!(f, [counter])
    /// });
    ///
    ///
    /// let formatted = format!(SimpleFormatContext::default(), [content])?;
    /// assert_eq!("Counter:\n\tCount: 0\nCount: 0\n", formatted.print()?.as_code());
    /// # Ok(())
    /// # }
    ///
    /// ```
    pub fn inspect(&mut self, f: &mut Formatter<'_, 'ast>) -> FormatResult<&[FormatElement<'ast>]> {
        let result = self.memory.get_or_init(|| f.intern(&self.inner));

        match result.as_ref() {
            Ok(Some(FormatElement::Interned(interned))) => Ok(interned),
            Ok(Some(other)) => Ok(std::slice::from_ref(other)),
            Ok(None) => Ok(&[]),
            Err(error) => Err(*error),
        }
    }
}

impl<'ast, F> Format<'ast> for Memoized<'ast, F>
where
    F: Format<'ast>,
{
    fn fmt(&self, f: &mut Formatter<'_, 'ast>) -> FormatResult<()> {
        let result = self.memory.get_or_init(|| f.intern(&self.inner));

        match result {
            Ok(Some(elements)) => {
                f.write_element(elements.clone())?;

                Ok(())
            }
            Ok(None) => Ok(()),
            Err(err) => Err(*err),
        }
    }
}
