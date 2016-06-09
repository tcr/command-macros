#![cfg_attr(
    feature = "nightly",
    feature(
        plugin_registrar,
        rustc_private,
        slice_patterns,
        question_mark,
        quote,
    ),
)]
#![cfg_attr(feature = "nightly", crate_type = "dylib")]

#[cfg(feature = "nightly")]
mod plugin;

#[cfg(feature = "nightly")] extern crate syntax;
#[cfg(feature = "nightly")] extern crate rustc;
#[cfg(feature = "nightly")] extern crate rustc_plugin;

#[cfg(feature = "nightly")]
use rustc_plugin::Registry;

#[cfg(feature = "nightly")]
#[plugin_registrar]
pub fn plugin_registrar(reg: &mut Registry) {
    reg.register_macro("command", plugin::expand_command);
}

#[macro_export]
macro_rules! cmd {
    ({$e:expr}) => ($e);

    // arg splice
    ({$e:expr} ($a:expr) $($tail:tt)*) => 
    {
        {
            let mut cmd = $e;
            cmd.arg(&$a);
            cmd!( {cmd} $($tail)* )
        }
    };

    // args splice
    ({$e:expr} [$aa:expr] $($tail:tt)*) => {
        {
            let mut cmd = $e;
            cmd.args(&$aa);
            cmd!( {cmd} $($tail)* )
        }
    };

    // match
    ({$e:expr} match ($m:expr) { $($($p:pat)|+ $(if $g:expr)* => {$($rr:tt)*} ),* } $($tail:tt)*) => {
        cmd!({$e} match ($m) { $($($p)|+ $(if $g)* => {$($rr)*})* } $($tail)*)
    };
    ({$e:expr} match ($m:expr) { $($($p:pat)|+ $(if $g:expr)* => {$($rr:tt)*},)* } $($tail:tt)*) => {
        cmd!({$e} match ($m) { $($($p)|+ $(if $g)* => {$($rr)*})* } $($tail)*)
    };
    ({$e:expr} match ($m:expr) { $($($p:pat)|+ $(if $g:expr)* => {$($rr:tt)*} )* } $($tail:tt)*) => {
        {
            let cmd = $e;
            cmd!( {match $m { $($($p)|+ $(if $g)* => cmd!({cmd} $($rr)*)),* }} $($tail)* )
        }
    };

    // if let
    ({$e:expr} if let $p:pat = ($m:expr) { $($then:tt)* } else { $($els:tt)* } $($tail:tt)*) => {
        {
            let cmd = $e;
            cmd!( {
                    if let $p = $m { cmd!({cmd} $($then)*) } else { cmd!({cmd} $($els)*) }
                  } $($tail)*)
        } 
    };
    ({$e:expr} if let $p:pat = ($m:expr) { $($then:tt)* } $($tail:tt)*) => {
        cmd!( {$e}if let $p = ($m) { $($then)* } else {} $($tail)* )
    };

    // if else
    ({$e:expr} if ($b:expr) { $($then:tt)* } else { $($els:tt)* } $($tail:tt)*) => {
        {
            let cmd = $e;
            cmd!( {
                    if $b { cmd!({cmd} $($then)*) } else { cmd!({cmd} $($els)*) }
                  } $($tail)*)
        } 
    };
    ({$e:expr} if ($b:expr) { $($then:tt)* } $($tail:tt)*) => {
        cmd!( {$e}if ($b) { $($then)* } else {} $($tail)* )
    };

    // naked ident
    ({$e:expr} $a:ident $($tail:tt)*) => (cmd!( {$e} (stringify!($a)) $($tail)* ));

    // Main entry points (command name)
    (($c:expr) $($tail:tt)*) => {
        cmd!( {::std::process::Command::new(&$c)} $($tail)* )
    };
    ($c:ident $($tail:tt)*) => (cmd!( (stringify!($c)) $($tail)* ));
}

#[cfg(test)]
use ::std::process::Command;

#[test]
fn expr() {
    let mut base: Command = cmd!(echo a);
    base.env("FOO", "bar");
    quicktest(cmd!({base} b), "a b");
}

#[cfg(test)]
fn quicktest(mut echocmd: Command, target: &str) {
    let out = echocmd.output().expect("quicktest: can't echo").stdout;
    assert_eq!(String::from_utf8_lossy(&out).trim(), target);
}

#[test]
fn simple() {
    let output = cmd!(echo raz dwa trzy).output().expect("can't echo");
    assert_eq!(output.stdout, &b"raz dwa trzy\n"[..]);
}

#[test]
fn ffmpeg() {
    let moreargs = ["-pix_fmt", "yuv420p"];
    let file = "file.mp4".to_string();
    let preset = "slow";
    let tmpname = "tmp.mkv";
    let output = cmd!(echo
            ("-i") (file)
            ("-c:v") libx264 ("-preset") (preset) [moreargs]
            ("-c:a") copy
            (tmpname))
        .output()
        .expect("can't echo");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "-i file.mp4 -c:v libx264 -preset slow -pix_fmt yuv420p -c:a copy tmp.mkv\n"
    );
}

#[test]
fn match_test() {
    let option = Some(5);

    quicktest(
        cmd!(echo
            match (option) {
                Some(x) => {("--number") (x.to_string())}
                None => {}
            }
            tail),
        "--number 5 tail"
    );

    for &(x, target) in &[
        (Ok(1), ". 0101 ."),
        (Ok(5), ". small 5 ."),
        (Ok(10), ". 10 ."),
        (Err("bu"), ". err bu ."),
    ] {
        quicktest(cmd!(
                echo (".") match (x) {
                    Ok(0) | Ok(1) => { ("0101") },
                    Ok(x) if x < 7 => { small (x.to_string()) },
                    Ok(x) => { (x.to_string()) },
                    Err(x) => { err (x) }
                } (".")
            ),
            target
        );
    }
}

#[test]
fn iflet() {
    let option = Some(5);
    quicktest(
        cmd!(echo
            if let Some(x) = (option) {("--number") (x.to_string())}
            tail),
        "--number 5 tail"
    );

    let option: Option<()> = None;
    quicktest(
        cmd!(echo
            if let Some(_) = (option) {} else { ok }
            tail),
        "ok tail"
    );
}


#[test]
fn ifelse() {
    quicktest(
        cmd!(echo
            if (true) { abc (1.to_string()) }
            tail),
        "abc 1 tail"
    );

    let counter = ::std::cell::Cell::new(0);
    quicktest(
        cmd!(echo
            ({counter.set(counter.get() + 1); "blah"})
            if (true) { a } else { b }
            if (false) { c } else { d }
            tail),
        "blah a d tail"
    );
    assert_eq!(counter.get(), 1);
}

#[test]
fn test_mutref() {
    let cmd = &mut Command::new("echo");
    let cmd: &mut Command = cmd!({cmd} foo);
    assert_eq!(cmd.output().unwrap().stdout, &b"foo\n"[..]);
}


