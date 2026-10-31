// The subset of `java.util.regex` the rule editor's "matches regex" text
// conditions use, as a whole-input match (`String.matches`): literals, `.`,
// classes with ranges and negation, `\d \w \s` (and their negations),
// groups, `(?:...)`, alternation, `^ $ \b`, and greedy or lazy `* + ? {n,m}`.
// Patterns outside the subset (lookaround, backreferences, possessive or
// atomic forms) are rejected like a `PatternSyntaxException`, so they never
// match. Backtracking is bounded so a pathological pattern fails instead of
// stalling the tick.
#[derive(Clone, Debug, PartialEq)]
enum Node {
    Char(char),
    Any,
    Class(Class),
    Start,
    End,
    Boundary(bool),
    Alt(Vec<Vec<Node>>),
    Repeat {
        node: Box<Node>,
        min: u32,
        max: Option<u32>,
        greedy: bool,
    },
}
#[derive(Clone, Debug, PartialEq)]
struct Class {
    negated: bool,
    items: Vec<Item>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Item {
    Range(char, char),
    Digit(bool),
    Word(bool),
    Space(bool),
}
impl Item {
    fn test(self, c: char) -> bool {
        match self {
            Self::Range(a, b) => (a..=b).contains(&c),
            Self::Digit(on) => c.is_ascii_digit() == on,
            Self::Word(on) => (c.is_ascii_alphanumeric() || c == '_') == on,
            Self::Space(on) => matches!(c, ' ' | '\t' | '\n' | '\x0b' | '\x0c' | '\r') == on,
        }
    }
}
impl Class {
    fn test(&self, c: char) -> bool {
        self.items.iter().any(|i| i.test(c)) != self.negated
    }
}

struct Parser<'a> {
    chars: &'a [char],
    at: usize,
}
impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }
    fn next(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += 1;
        Some(c)
    }
    fn eat(&mut self, c: char) -> bool {
        let hit = self.peek() == Some(c);
        if hit {
            self.at += 1;
        }
        hit
    }
    fn alternation(&mut self) -> Result<Node, ()> {
        let mut branches = vec![self.sequence()?];
        while self.eat('|') {
            branches.push(self.sequence()?);
        }
        Ok(Node::Alt(branches))
    }
    fn sequence(&mut self) -> Result<Vec<Node>, ()> {
        let mut out = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            out.push(self.quantified(atom)?);
        }
        Ok(out)
    }
    fn number(&mut self) -> Option<u32> {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        self.chars[start..self.at]
            .iter()
            .collect::<String>()
            .parse()
            .ok()
    }
    fn quantified(&mut self, atom: Node) -> Result<Node, ()> {
        let (min, max) = match self.peek() {
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('?') => (0, Some(1)),
            Some('{') => {
                let save = self.at;
                self.at += 1;
                let Some(min) = self.number() else {
                    // A brace that does not start a bound is an error in Java.
                    self.at = save;
                    return Err(());
                };
                let max = if self.eat(',') {
                    if self.peek() == Some('}') {
                        None
                    } else {
                        Some(self.number().ok_or(())?)
                    }
                } else {
                    Some(min)
                };
                if !self.eat('}') || max.is_some_and(|m| m < min) {
                    return Err(());
                }
                self.at -= 1;
                (min, max)
            }
            _ => return Ok(atom),
        };
        self.at += 1;
        if matches!(atom, Node::Start | Node::End | Node::Boundary(_)) {
            return Err(());
        }
        let greedy = !self.eat('?');
        if self.peek() == Some('+') {
            return Err(());
        }
        let node = Node::Repeat {
            node: Box::new(atom),
            min,
            max,
            greedy,
        };
        if matches!(self.peek(), Some('*' | '+' | '?' | '{')) {
            return Err(());
        }
        Ok(node)
    }
    fn escape(&mut self) -> Result<Result<Item, char>, ()> {
        Ok(match self.next().ok_or(())? {
            'd' => Ok(Item::Digit(true)),
            'D' => Ok(Item::Digit(false)),
            'w' => Ok(Item::Word(true)),
            'W' => Ok(Item::Word(false)),
            's' => Ok(Item::Space(true)),
            'S' => Ok(Item::Space(false)),
            't' => Err('\t'),
            'n' => Err('\n'),
            'r' => Err('\r'),
            'f' => Err('\x0c'),
            c if c.is_ascii_alphanumeric() => return Err(()),
            c => Err(c),
        })
    }
    fn atom(&mut self) -> Result<Node, ()> {
        match self.next().ok_or(())? {
            '.' => Ok(Node::Any),
            '^' => Ok(Node::Start),
            '$' => Ok(Node::End),
            '(' => {
                if self.eat('?') {
                    // `(?i)` is a no-op: both sides are already lowercase.
                    if self.chars[self.at..].starts_with(&['i', ')']) {
                        self.at += 2;
                        return Ok(Node::Alt(vec![Vec::new()]));
                    }
                    if !self.eat(':') {
                        return Err(());
                    }
                }
                let inner = self.alternation()?;
                if !self.eat(')') {
                    return Err(());
                }
                Ok(inner)
            }
            '[' => self.class().map(Node::Class),
            '\\' => match self.peek() {
                Some('b') => {
                    self.at += 1;
                    Ok(Node::Boundary(true))
                }
                Some('B') => {
                    self.at += 1;
                    Ok(Node::Boundary(false))
                }
                _ => Ok(match self.escape()? {
                    Ok(item) => Node::Class(Class {
                        negated: false,
                        items: vec![item],
                    }),
                    Err(c) => Node::Char(c),
                }),
            },
            '*' | '+' | '?' | ')' => Err(()),
            '{' => Err(()),
            c => Ok(Node::Char(c)),
        }
    }
    fn class(&mut self) -> Result<Class, ()> {
        let negated = self.eat('^');
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let c = self.next().ok_or(())?;
            if c == ']' && !first {
                break;
            }
            first = false;
            let low = match c {
                '\\' => match self.escape()? {
                    Ok(item) => {
                        items.push(item);
                        continue;
                    }
                    Err(c) => c,
                },
                '[' => return Err(()),
                c => c,
            };
            if self.peek() == Some('-') && self.chars.get(self.at + 1).is_some_and(|c| *c != ']') {
                self.at += 1;
                let high = match self.next().ok_or(())? {
                    '\\' => match self.escape()? {
                        Err(c) => c,
                        Ok(_) => return Err(()),
                    },
                    c => c,
                };
                if high < low {
                    return Err(());
                }
                items.push(Item::Range(low, high));
            } else {
                items.push(Item::Range(low, low));
            }
        }
        Ok(Class { negated, items })
    }
}

/// A compiled pattern.
#[derive(Clone, Debug, PartialEq)]
pub struct Regex(Node);

impl Regex {
    pub fn new(pattern: &str) -> Option<Self> {
        let chars: Vec<char> = pattern.chars().collect();
        let mut parser = Parser {
            chars: &chars,
            at: 0,
        };
        let node = parser.alternation().ok()?;
        (parser.at == chars.len()).then_some(Self(node))
    }
    /// `String.matches`: the whole input.
    pub fn matches(&self, input: &str) -> bool {
        let chars: Vec<char> = input.chars().collect();
        let m = Matcher {
            input: &chars,
            budget: std::cell::Cell::new(200_000),
        };
        let len = chars.len();
        m.node(&self.0, 0, &mut |p| p == len)
    }
}

struct Matcher<'a> {
    input: &'a [char],
    budget: std::cell::Cell<u32>,
}
impl Matcher<'_> {
    fn word(&self, at: usize) -> bool {
        self.input
            .get(at)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
    }
    fn node(&self, node: &Node, at: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        let Some(left) = self.budget.get().checked_sub(1) else {
            return false;
        };
        self.budget.set(left);
        let one = |test: &dyn Fn(char) -> bool| self.input.get(at).is_some_and(|c| test(*c));
        match node {
            Node::Char(c) => one(&|x| x == *c) && k(at + 1),
            Node::Any => one(&|x| x != '\n' && x != '\r') && k(at + 1),
            Node::Class(class) => one(&|x| class.test(x)) && k(at + 1),
            Node::Start => at == 0 && k(at),
            Node::End => at == self.input.len() && k(at),
            Node::Boundary(on) => {
                let edge = at > 0 && self.word(at - 1);
                (edge != self.word(at)) == *on && k(at)
            }
            Node::Alt(branches) => branches.iter().any(|b| self.sequence(b, at, k)),
            Node::Repeat {
                node,
                min,
                max,
                greedy,
            } => self.repeat(node, (*min, *max, *greedy), 0, at, k),
        }
    }
    fn sequence(&self, nodes: &[Node], at: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        match nodes.split_first() {
            None => k(at),
            Some((first, rest)) => self.node(first, at, &mut |p| self.sequence(rest, p, k)),
        }
    }
    fn repeat(
        &self,
        node: &Node,
        bounds: (u32, Option<u32>, bool),
        count: u32,
        at: usize,
        k: &mut dyn FnMut(usize) -> bool,
    ) -> bool {
        let (min, max, greedy) = bounds;
        let more = max.is_none_or(|m| count < m);
        let stop = count >= min;
        let again = |k: &mut dyn FnMut(usize) -> bool| {
            more && self.node(node, at, &mut |p| {
                // An empty iteration past the minimum cannot make progress.
                (p != at || count < min) && self.repeat(node, bounds, count + 1, p, k)
            })
        };
        // Greedy tries one more iteration first; lazy tries stopping first.
        [greedy, !greedy]
            .into_iter()
            .any(|iterate| if iterate { again(k) } else { stop && k(at) })
    }
}

#[cfg(test)]
mod tests {
    use super::Regex;

    fn m(pattern: &str, input: &str) -> bool {
        Regex::new(pattern).expect(pattern).matches(input)
    }
    #[test]
    fn whole_input_matches() {
        assert!(m("diamond.*", "diamond sword"));
        assert!(!m("sword", "diamond sword"));
        assert!(m(".*sword", "diamond sword"));
        assert!(m("(iron|diamond) (sword|axe)", "iron axe"));
        assert!(!m("(iron|diamond) (sword|axe)", "gold axe"));
        assert!(m("[a-z]+ ?\\d{1,2}", "level 12"));
        assert!(!m("[a-z]+ ?\\d{1,2}", "level 123"));
        assert!(m("[^0-9]*", "no digits"));
        assert!(!m("[^0-9]*", "1 digit"));
        assert!(m("a.*?b", "axxb"));
        assert!(m("(?:ab)+", "ababab"));
        assert!(m("^\\w+\\s\\w+$", "golden apple"));
        assert!(m(".*\\bapple\\b.*", "golden apple pie"));
        assert!(!m(".*\\bapple\\b.*", "pineapples"));
        assert!(m("a{2,}", "aaaa"));
        assert!(!m("a{2,3}", "aaaa"));
        assert!(m("(a*)*", "aaaa"));
        assert!(m("[.]x", ".x"));
        assert!(m("\\.x", ".x"));
        assert!(!m("\\.x", "ax"));
        assert!(m("(?i)golden.*", "golden apple"));
    }
    #[test]
    fn unsupported_or_invalid_patterns_are_rejected() {
        for bad in [
            "(", "a)", "*a", "[a", "a{3,2}", "(?=a)", "a++", "(?<n>a)", "\\1", "a{x}",
        ] {
            assert!(Regex::new(bad).is_none(), "{bad}");
        }
    }
    #[test]
    fn pathological_patterns_give_up() {
        let input = "a".repeat(40) + "b";
        assert!(!m("(a|a)*c", &input));
    }
}
