use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::*;

const INFO: &str = "<info><name>Test</name><author>A</author><copyright>C</copyright>\
                    <date>D</date><description>X</description></info>";

const GEOMETRY: &str = r#"<frame_geometry name="g">
<distance name="left_width" value="1"/>
<distance name="right_width" value="2"/>
<distance name="bottom_height" value="3"/>
<distance name="title_vertical_pad" value="4"/>
<distance name="left_titlebar_edge" value="5"/>
<distance name="right_titlebar_edge" value="6"/>
<aspect_ratio name="button" value="1.0"/>
<border name="title_border" left="1" right="2" top="3" bottom="4"/>
<border name="button_border" left="0" right="0" top="0" bottom="0"/>
</frame_geometry>"#;

const WINDOWS: &str = r#"<window type="normal" style_set="set"/>
<window type="dialog" style_set="set"/>
<window type="modal_dialog" style_set="set"/>
<window type="menu" style_set="set"/>
<window type="utility" style_set="set"/>
<window type="border" style_set="set"/>"#;

const STYLE_SET: &str = r#"<frame_style_set name="set">
<frame focus="yes" state="normal" resize="both" style="s"/>
<frame focus="no" state="normal" resize="both" style="s"/>
<frame focus="yes" state="maximized" style="s"/>
<frame focus="no" state="maximized" style="s"/>
<frame focus="yes" state="shaded" style="s"/>
<frame focus="no" state="shaded" style="s"/>
<frame focus="yes" state="maximized_and_shaded" style="s"/>
<frame focus="no" state="maximized_and_shaded" style="s"/>
</frame_style_set>"#;

/// Normal and pressed draw ops for every button a style must define in `major`'s format.
fn buttons(major: u32) -> String {
    let mut functions = vec!["close", "maximize", "minimize", "menu"];
    if major >= 2 {
        functions.extend(["shade", "above", "stick", "unshade", "unabove", "unstick"]);
    }
    functions
        .iter()
        .flat_map(|function| {
            ["normal", "pressed"].map(|state| {
                format!(r#"<button function="{function}" state="{state}" draw_ops="empty"/>"#)
            })
        })
        .collect()
}

/// The smallest theme Marco accepts in `major`'s format, with `extra` before the
/// `<window>` table.
fn document(major: u32, extra: &str) -> String {
    format!(
        "<metacity_theme>\n{INFO}\n{GEOMETRY}\n<draw_ops name=\"empty\"/>\n\
         <frame_style name=\"s\" geometry=\"g\">\n{}\n</frame_style>\n{STYLE_SET}\n{extra}\n\
         {WINDOWS}\n</metacity_theme>\n",
        buttons(major)
    )
}

fn parse_at(text: &str, major: u32, path: &Path) -> Result<Theme, ThemeError> {
    Theme::parse(
        text.as_bytes(),
        "Test",
        &path.join(format!("metacity-theme-{major}.xml")),
        major,
    )
}

fn parse(text: &str, major: u32) -> Result<Theme, ThemeError> {
    parse_at(text, major, Path::new("/themes/Test/metacity-1"))
}

fn load(major: u32, extra: &str) -> Theme {
    parse(&document(major, extra), major).unwrap_or_else(|e| panic!("{e}"))
}

fn reject_text(text: &str, major: u32) -> (usize, String) {
    match parse(text, major) {
        Err(ThemeError::Invalid { line, message, .. }) => (line, message),
        other => panic!("expected a rejection, got {other:?}"),
    }
}

/// The message Marco would reject `extra` with, in a v3 theme.
#[track_caller]
fn reject(extra: &str) -> String {
    reject_text(&document(3, extra), 3).1
}

#[track_caller]
fn assert_rejects(extra: &str, expected: &str) {
    let message = reject(extra);
    assert!(
        message.contains(expected),
        "{extra}\nwas rejected with {message:?}, expected {expected:?}"
    );
}

fn replace(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "{from:?} isn't in the document");
    text.replacen(from, to, 1)
}

#[test]
fn the_smallest_theme_of_each_format_loads() {
    for major in 1..=3 {
        let theme = load(major, "");
        assert_eq!(theme.format_version, major * 1000);
        assert_eq!(theme.info.name, "Test");
        assert_eq!(theme.info.description, "X");
    }
}

// Leniencies Marco has that 04 found real themes relying on.

#[test]
fn a_raw_open_angle_in_a_version_check_is_accepted() {
    let theme = load(3, r#"<constant name="Old" value="1" version="< 3.1"/>"#);
    assert!(!theme.constants.ints.contains_key("Old"));
}

#[test]
fn attributes_without_space_between_them_are_accepted() {
    let theme = load(3, r#"<constant version=">= 3.3"name="New" value="2"/>"#);
    assert_eq!(theme.constants.ints["New"], 2);
}

#[test]
fn an_element_whose_version_check_fails_is_skipped_with_its_children() {
    let theme = load(
        3,
        r#"<draw_ops name="future" version=">= 4.0"><nonsense/></draw_ops>
           <draw_ops name="now" version="&lt;= 3.5"/>"#,
    );
    assert!(!theme.draw_ops.contains_key("future"));
    assert!(theme.draw_ops.contains_key("now"));
}

#[test]
fn a_version_check_on_the_root_raises_the_format_version() {
    let text = replace(
        &document(
            3,
            r#"<draw_ops name="d"><title color="black" x="0" y="0" ellipsize_width="width"/></draw_ops>"#,
        ),
        "<metacity_theme>",
        r#"<metacity_theme version=">= 3.1">"#,
    );
    assert_eq!(parse(&text, 3).unwrap().format_version, 3001);
}

#[test]
fn a_root_version_check_marco_fails_is_reported_as_unsupported() {
    let text = replace(
        &document(3, ""),
        "<metacity_theme>",
        r#"<metacity_theme version=">= 3.6">"#,
    );
    assert!(matches!(
        parse(&text, 3),
        Err(ThemeError::UnsupportedVersion { .. })
    ));
}

#[test]
fn v1_files_accept_the_v2_extensions_marco_never_gates() {
    // META_THEME_ALLOWS compares the file's version, 1000 or more, with 2.
    let theme = load(
        1,
        r#"<constant name="Dark" value="black"/>
           <frame_geometry name="round" parent="g" rounded_top_left="4" hide_buttons="true"/>
           <draw_ops name="d">
             <arc color="Dark" x="0" y="0" width="4" height="4" from="0" to="90"/>
           </draw_ops>"#,
    );
    let round = &theme.geometries["round"];
    assert_eq!(round.top_left_radius, 4);
    assert!(round.hide_buttons);
    let DrawOp::Arc { color, .. } = &theme.draw_ops["d"].ops[0] else {
        panic!()
    };
    assert_eq!(color, &ColorSpec::parse("black").unwrap());
}

#[test]
fn menu_icon_fallback_shadow_and_padding_are_parsed_and_ignored() {
    load(
        3,
        r#"<menu_icon function="close" state="normal">
             <draw_ops><line color="black" x1="0" y1="0" x2="1" y2="1"/></draw_ops>
           </menu_icon>
           <fallback icon="a.png" mini_icon="b.png"/>
           <frame_style name="extras" parent="s"><shadow radius="2"/><padding left="1"/></frame_style>"#,
    );
}

#[test]
fn theme_icon_images_are_parsed_and_ignored() {
    let theme = load(
        3,
        r#"<draw_ops name="d"><image filename="theme:window-close" x="0" y="0" width="16" height="16"/></draw_ops>"#,
    );
    let DrawOp::Image { source, .. } = &theme.draw_ops["d"].ops[0] else {
        panic!()
    };
    assert_eq!(source, &ImageSource::IconTheme("window-close".to_owned()));
}

// The model.

#[test]
fn a_geometry_copies_its_parent_and_overrides_it() {
    let theme = load(
        3,
        r#"<frame_geometry name="child" parent="g" has_title="false" title_scale="small">
             <distance name="left_width" value="9"/>
           </frame_geometry>"#,
    );
    let child = &theme.geometries["child"];
    assert_eq!(child.left_width, 9);
    assert_eq!(child.right_width, 2);
    assert!(!child.has_title);
    assert_eq!(child.title_scale, 0.8333333333333);
    assert_eq!(
        child.title_border,
        Border {
            left: 1,
            right: 2,
            top: 3,
            bottom: 4
        }
    );
    assert_eq!(child.button_sizing, ButtonSizing::Aspect(1.0));
}

#[test]
fn buttons_fall_back_through_parents_backgrounds_and_prelight() {
    let text = replace(
        &document(3, ""),
        r#"<frame_style name="s" geometry="g">"#,
        &format!(
            r#"<draw_ops name="middle"><rectangle color="black" x="0" y="0" width="1" height="1"/></draw_ops>
               <frame_style name="parent" geometry="g">
                 <button function="left_middle_background" state="normal" draw_ops="middle"/>{}
               </frame_style>
               <frame_style name="s" parent="parent">"#,
            buttons(3)
        ),
    );
    let theme = parse(&text, 3).unwrap();
    let style = &theme.styles["s"];
    let middle = &theme.draw_ops["middle"];
    let background = style
        .button(ButtonType::LeftSingleBackground, ButtonState::Prelight)
        .unwrap();
    assert!(Arc::ptr_eq(background, middle));
    let close = style
        .button(ButtonType::Close, ButtonState::Prelight)
        .unwrap();
    assert!(Arc::ptr_eq(close, &theme.draw_ops["empty"]));
    assert!(
        style
            .button(ButtonType::RightLeftBackground, ButtonState::Normal)
            .is_none()
    );
    assert!(Arc::ptr_eq(&style.geometry, &theme.geometries["g"]));
}

#[test]
fn pieces_come_from_the_nearest_style_that_has_them() {
    let text = replace(
        &document(3, ""),
        "</frame_style>",
        r#"<piece position="titlebar" draw_ops="empty"/></frame_style>
           <frame_style name="child" parent="s">
             <piece position="overlay"><draw_ops><clip x="0" y="0" width="1" height="1"/></draw_ops></piece>
           </frame_style>"#,
    );
    let theme = parse(&text, 3).unwrap();
    let child = &theme.styles["child"];
    assert!(Arc::ptr_eq(
        child.piece(FramePiece::Titlebar).unwrap(),
        &theme.draw_ops["empty"]
    ));
    assert_eq!(child.piece(FramePiece::Overlay).unwrap().ops.len(), 1);
    assert!(child.piece(FramePiece::Title).is_none());
}

#[test]
fn style_sets_fall_back_as_marco_does() {
    let text = replace(
        &document(3, ""),
        "</frame_style>",
        r#"</frame_style><frame_style name="tiled" parent="s"/>"#,
    );
    let text = replace(
        &text,
        r#"<frame focus="yes" state="maximized" style="s"/>"#,
        r#"<frame focus="yes" state="maximized" style="s"/>
           <frame focus="yes" state="tiled_left" style="tiled"/>"#,
    );
    let text = replace(
        &text,
        r#"<window type="border" style_set="set"/>"#,
        r#"<frame_style_set name="border" parent="set"/>
           <window type="border" style_set="border"/>"#,
    );
    let theme = parse(&text, 3).unwrap();
    let style =
        |frame_type, state, focus| theme.frame_style(frame_type, state, FrameResize::None, focus);
    let s = &theme.styles["s"];
    let tiled = &theme.styles["tiled"];
    assert!(Arc::ptr_eq(
        style(FrameType::Normal, FrameState::Normal, FrameFocus::Yes),
        s
    ));
    assert!(Arc::ptr_eq(
        style(FrameType::Normal, FrameState::TiledLeft, FrameFocus::Yes),
        tiled
    ));
    assert!(Arc::ptr_eq(
        style(FrameType::Normal, FrameState::TiledLeft, FrameFocus::No),
        s
    ));
    assert!(Arc::ptr_eq(
        style(
            FrameType::Normal,
            FrameState::TiledRightAndShaded,
            FrameFocus::No
        ),
        s
    ));
    assert!(Arc::ptr_eq(
        theme.style_set(FrameType::Attached),
        &theme.style_sets["border"]
    ));
}

#[test]
fn includes_share_the_list_they_name() {
    let theme = load(
        3,
        r#"<draw_ops name="a"><clip x="0" y="0" width="1" height="1"/></draw_ops>
           <draw_ops name="b"><include name="a"/><include name="a" x="1" width="width - 1"/></draw_ops>"#,
    );
    let ops = &theme.draw_ops["b"].ops;
    let DrawOp::Include { list, rect } = &ops[0] else {
        panic!()
    };
    assert!(Arc::ptr_eq(list, &theme.draw_ops["a"]));
    assert_eq!(rect.x, Expr::Int(0));
    assert_eq!(rect.width, Expr::Variable(Variable::Width));
    let DrawOp::Include { rect, .. } = &ops[1] else {
        panic!()
    };
    assert_eq!(rect.x, Expr::Int(1));
}

#[test]
fn draw_ops_hold_parsed_expressions_and_colours() {
    let theme = load(
        3,
        r#"<constant name="Pad" value="2"/>
           <constant name="Accent" value="gtk:custom(wm_bg,#ffffff)"/>
           <draw_ops name="d">
             <line color="Accent" x1="Pad" y1="0" x2="Pad" y2="height - Later" width="2"/>
             <gradient type="vertical" x="0" y="0" width="width" height="height" alpha="0.5:1.0">
               <color value="gtk:bg[NORMAL]"/><color value="blend/white/black/0.5"/>
             </gradient>
             <tint color="black" x="0" y="0" width="1" height="1" alpha="0.5"/>
           </draw_ops>
           <constant name="Later" value="1"/>"#,
    );
    let ops = &theme.draw_ops["d"].ops;
    let DrawOp::Line {
        color,
        x1,
        x2,
        y2,
        width,
        ..
    } = &ops[0]
    else {
        panic!()
    };
    assert_eq!(
        color,
        &ColorSpec::parse("gtk:custom(wm_bg,#ffffff)").unwrap()
    );
    assert_eq!(x1, &Expr::Int(2));
    assert_eq!(x2, &None);
    // Marco substitutes constants when it parses an expression, so a later one is an
    // unknown variable.
    assert!(matches!(y2, Some(Expr::Invalid(_))));
    assert_eq!(*width, 2);
    let DrawOp::Gradient {
        kind,
        colors,
        alpha,
        ..
    } = &ops[1]
    else {
        panic!()
    };
    assert_eq!(*kind, GradientType::Vertical);
    assert_eq!(colors.len(), 2);
    assert_eq!(alpha, &Some(AlphaGradient(vec![127, 255])));
    let DrawOp::Tint { alpha, .. } = &ops[2] else {
        panic!()
    };
    assert_eq!(alpha, &AlphaGradient(vec![127]));
}

#[test]
fn arc_angles_are_stored_as_marco_converts_them() {
    let theme = load(
        3,
        r#"<draw_ops name="d">
             <arc color="black" x="0" y="0" width="4" height="4" start_angle="30" extent_angle="60"/>
             <arc color="black" x="0" y="0" width="4" height="4" from="0" to="90"/>
           </draw_ops>"#,
    );
    let angles: Vec<(f64, f64)> = theme.draw_ops["d"]
        .ops
        .iter()
        .map(|op| match op {
            DrawOp::Arc {
                start_angle,
                extent_angle,
                ..
            } => (*start_angle, *extent_angle),
            _ => panic!(),
        })
        .collect();
    assert_eq!(angles, [(30.0, 60.0), (0.5, 0.25 - 0.5)]);
}

#[test]
fn images_are_found_in_the_theme_directory() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("close.png"), b"").unwrap();
    let text = document(
        3,
        r#"<draw_ops name="d"><image filename="close.png" x="0" y="0" width="object_width" height="object_height" colorize="white" fill_type="tile"/></draw_ops>"#,
    );
    let theme = parse_at(&text, 3, dir.path()).unwrap();
    let DrawOp::Image {
        source,
        colorize,
        fill,
        rect,
        ..
    } = &theme.draw_ops["d"].ops[0]
    else {
        panic!()
    };
    assert_eq!(source, &ImageSource::File(dir.path().join("close.png")));

    // g_build_filename keeps an absolute filename inside the theme's directory.
    let text = replace(&text, r#"filename="close.png""#, r#"filename="/close.png""#);
    let theme = parse_at(&text, 3, dir.path()).unwrap();
    let DrawOp::Image { source, .. } = &theme.draw_ops["d"].ops[0] else {
        panic!()
    };
    assert_eq!(source, &ImageSource::File(dir.path().join("close.png")));
    assert!(colorize.is_some());
    assert_eq!(*fill, ImageFill::Tile);
    assert_eq!(rect.width, Expr::Variable(Variable::ObjectWidth));
}

// Rejections: one per class of error Marco reports.

#[test]
fn a_rejection_names_the_file_and_line() {
    let text = document(3, "<bogus/>");
    let bogus_line = text[..text.find("<bogus/>").unwrap()].matches('\n').count() + 1;
    let Err(ThemeError::Invalid {
        path,
        line,
        message,
    }) = parse(&text, 3)
    else {
        panic!()
    };
    assert_eq!(
        path,
        PathBuf::from("/themes/Test/metacity-1/metacity-theme-3.xml")
    );
    assert_eq!(line, bogus_line);
    assert!(message.contains("<bogus> is not allowed below <metacity_theme>"));
    let shown = ThemeError::Invalid {
        path,
        line,
        message,
    }
    .to_string();
    let prefix = format!("/themes/Test/metacity-1/metacity-theme-3.xml:{bogus_line}: ");
    assert!(shown.starts_with(&prefix), "{shown}");
}

#[test]
fn malformed_markup_is_rejected() {
    let (line, message) = reject_text("<metacity_theme>\n<info>\n</metacity_theme>", 3);
    assert_eq!(line, 3);
    assert!(
        message.contains("currently open element is “info”"),
        "{message}"
    );
}

#[test]
fn the_root_element_must_be_metacity_theme() {
    let (_, message) = reject_text("<theme/>", 3);
    assert!(message.contains("Outermost element in theme must be <metacity_theme> not <theme>"));
}

#[test]
fn elements_out_of_place_are_rejected() {
    assert_rejects(
        "<name>x</name>",
        "<name> is not allowed below <metacity_theme>",
    );
    assert_rejects(
        r#"<constant name="A" value="1"><x/></constant>"#,
        "not allowed inside a <constant> element",
    );
    assert_rejects(
        r#"<draw_ops name="d"><line color="black" x1="0" y1="0" x2="0" y2="0"><x/></line></draw_ops>"#,
        "not allowed inside a draw operation element",
    );
    assert_rejects(
        r#"<draw_ops name="d"><gradient type="vertical" x="0" y="0" width="1" height="1"><stop/></gradient></draw_ops>"#,
        "<stop> is not allowed below <gradient>",
    );
    assert_rejects(
        r#"<fallback><x/></fallback>"#,
        "inside a <fallback> element",
    );
}

#[test]
fn unknown_repeated_and_missing_attributes_are_rejected() {
    assert_rejects(
        r#"<constant name="A" value="1" colour="red"/>"#,
        "Attribute \"colour\" is invalid on <constant> element",
    );
    assert_rejects(
        r#"<constant name="A" name="B" value="1"/>"#,
        "Attribute \"name\" repeated twice on the same <constant> element",
    );
    assert_rejects(
        r#"<constant name="A"/>"#,
        "No \"value\" attribute on element <constant>",
    );
    assert_rejects(
        r#"<info name="x"/>"#,
        "Attribute \"name\" is invalid on <info>",
    );
}

#[test]
fn text_inside_an_element_is_rejected() {
    assert_rejects(
        r#"<draw_ops name="d">x</draw_ops>"#,
        "No text is allowed inside element <draw_ops>",
    );
}

#[test]
fn info_is_required_once_each() {
    let text = replace(&document(3, ""), "<author>A</author>", "");
    assert!(
        reject_text(&text, 3)
            .1
            .contains("No <author> set for theme \"Test\"")
    );
    let text = replace(
        &document(3, ""),
        "<date>D</date>",
        "<date>D</date><date>E</date>",
    );
    assert!(
        reject_text(&text, 3)
            .1
            .contains("<date> specified twice for this theme")
    );
}

#[test]
fn window_types_are_checked() {
    let text = replace(
        &document(3, ""),
        r#"<window type="menu" style_set="set"/>"#,
        "",
    );
    assert!(
        reject_text(&text, 3)
            .1
            .contains("No frame style set for window type \"menu\"")
    );
    assert_rejects(
        r#"<frame_style_set name="x" parent="set"/><window type="normal" style_set="x"/>"#,
        "Window type \"normal\" has already been assigned a style set",
    );
    assert_rejects(
        r#"<window type="toolbar" style_set="set"/>"#,
        "Unknown type \"toolbar\"",
    );
    assert_rejects(
        r#"<window type="normal" style_set="nope"/>"#,
        "Unknown style_set \"nope\"",
    );
}

#[test]
fn features_newer_than_the_elements_version_are_rejected() {
    // attached needs 3.2, and the file declares nothing beyond 3.0.
    let text = replace(
        &document(3, ""),
        "</metacity_theme>",
        r#"<window type="attached" style_set="set"/></metacity_theme>"#,
    );
    assert!(
        reject_text(&text, 3)
            .1
            .contains("Unknown type \"attached\"")
    );
    let text = replace(
        &text,
        r#"<window type="attached""#,
        r#"<window version=">= 3.2" type="attached""#,
    );
    parse(&text, 3).unwrap();
    assert_rejects(
        r#"<draw_ops name="d"><title color="black" x="0" y="0" ellipsize_width="width"/></draw_ops>"#,
        "No \"ellipsize_width\" attribute on element <title>",
    );
    let text = replace(
        &document(1, ""),
        "</frame_style>",
        r#"<button function="shade" state="normal" draw_ops="empty"/></frame_style>"#,
    );
    assert!(
        reject_text(&text, 1)
            .1
            .contains("Button function \"shade\" does not exist in this version (1000, need 2000)")
    );
}

#[test]
fn version_checks_are_checked() {
    let text = replace(
        &document(2, ""),
        "<metacity_theme>",
        r#"<metacity_theme version=">= 2.0">"#,
    );
    assert!(reject_text(&text, 2).1.contains(
        "\"version\" attribute cannot be used in metacity-theme-1.xml or metacity-theme-2.xml"
    ));
    assert_rejects(
        r#"<constant version="3.0" name="A" value="1"/>"#,
        "Bad version specification '3.0'",
    );
}

#[test]
fn constants_are_checked() {
    assert_rejects(
        r#"<constant name="small" value="1"/>"#,
        "User-defined constants must begin with a capital letter; \"small\" does not",
    );
    assert_rejects(
        r#"<constant name="A" value="1"/><constant name="A" value="2"/>"#,
        "Constant \"A\" has already been defined",
    );
    assert_rejects(
        r#"<constant name="A" value="1.5x"/>"#,
        "Did not understand trailing characters \"x\" in string \"1.5x\"",
    );
    // A hex float is a float to g_ascii_strtod.
    let theme = load(3, r#"<constant name="Hex" value="0x1.8"/>"#);
    assert_eq!(theme.constants.floats["Hex"], 1.5);
}

#[test]
fn numbers_and_keywords_are_checked() {
    let distance = |value: &str| {
        format!(
            r#"<frame_geometry name="x" parent="g"><distance name="left_width" value="{value}"/></frame_geometry>"#
        )
    };
    assert_rejects(&distance("wide"), "Could not parse \"wide\" as an integer");
    assert_rejects(
        &distance("3px"),
        "Did not understand trailing characters \"px\" in string \"3px\"",
    );
    assert_rejects(&distance("-1"), "Integer -1 must be positive");
    assert_rejects(
        &distance("5000"),
        "Integer 5000 is too large, current max is 4096",
    );
    assert_rejects(
        r#"<frame_geometry name="x" parent="g" has_title="yes"/>"#,
        "Boolean values must be \"true\" or \"false\" not \"yes\"",
    );
    assert_rejects(
        r#"<frame_geometry name="x" parent="g" title_scale="huge"/>"#,
        "Invalid title scale \"huge\"",
    );
    assert_rejects(
        r#"<draw_ops name="d"><tint color="black" x="0" y="0" width="1" height="1" alpha="1.5"/></draw_ops>"#,
        "Alpha must be between 0.0 (invisible) and 1.0 (fully opaque), was 1.5",
    );
    assert_rejects(
        r#"<draw_ops name="d"><image filename="theme:x" x="0" y="0" width="1" height="1" fill_type="stretch"/></draw_ops>"#,
        "Did not understand fill type \"stretch\" for <image> element",
    );
}

#[test]
fn geometries_are_checked() {
    assert_rejects(
        r#"<frame_geometry name="g"/>"#,
        "<frame_geometry> name \"g\" used a second time",
    );
    assert_rejects(
        r#"<frame_geometry name="x" parent="y"/>"#,
        "<frame_geometry> parent \"y\" has not been defined",
    );
    assert_rejects(
        r#"<frame_geometry name="x"/>"#,
        "frame geometry does not specify \"left_width\" dimension",
    );
    let without = |line: &str| replace(&document(3, ""), line, "");
    let text = without(r#"<border name="title_border" left="1" right="2" top="3" bottom="4"/>"#);
    assert!(
        reject_text(&text, 3).1.contains(
            "frame geometry does not specify dimension \"top\" for border \"title_border\""
        )
    );
    let text = without(r#"<aspect_ratio name="button" value="1.0"/>"#);
    assert!(
        reject_text(&text, 3)
            .1
            .contains("Frame geometry does not specify size of buttons")
    );
    let text = replace(&document(3, ""), r#"value="1.0"/>"#, r#"value="20.0"/>"#);
    assert!(
        reject_text(&text, 3)
            .1
            .contains("Button aspect ratio 20 is not reasonable")
    );
    assert_rejects(
        r#"<frame_geometry name="x" parent="g"><distance name="button_width" value="1"/></frame_geometry>"#,
        "Cannot specify both \"button_width\"/\"button_height\" and \"aspect_ratio\" for buttons",
    );
    assert_rejects(
        r#"<frame_geometry name="x" parent="g"><distance name="top_height" value="1"/></frame_geometry>"#,
        "Distance \"top_height\" is unknown",
    );
    assert_rejects(
        r#"<frame_geometry name="x" parent="g"><border name="frame_border" left="1" right="1" top="1" bottom="1"/></frame_geometry>"#,
        "Border \"frame_border\" is unknown",
    );
    assert_rejects(
        r#"<frame_geometry name="x" parent="g"><aspect_ratio name="title" value="1.0"/></frame_geometry>"#,
        "Aspect ratio \"title\" is unknown",
    );
}

#[test]
fn draw_ops_are_checked() {
    let ops = |body: &str| format!(r#"<draw_ops name="d">{body}</draw_ops>"#);
    assert_rejects(
        r#"<draw_ops name="empty"/>"#,
        "<draw_ops> name \"empty\" used a second time",
    );
    assert_rejects(
        &ops(r#"<include name="nope"/>"#),
        "No <draw_ops> called \"nope\" has been defined",
    );
    assert_rejects(
        &ops(r#"<include name="d"/>"#),
        "Including draw_ops \"d\" here would create a circular reference",
    );
    assert_rejects(
        &ops(r#"<tile name="empty" tile_width="1"/>"#),
        "No \"tile_height\" attribute on element <tile>",
    );
    assert_rejects(
        &ops(
            r#"<gradient type="vertical" x="0" y="0" width="1" height="1"><color value="black"/></gradient>"#,
        ),
        "Gradients should have at least two colors",
    );
    assert_rejects(
        &ops(r#"<gradient type="radial" x="0" y="0" width="1" height="1"/>"#),
        "Did not understand value \"radial\" for type of gradient",
    );
    assert_rejects(
        &ops(r#"<rectangle color="gtk:bg[SHINY]" x="0" y="0" width="1" height="1"/>"#),
        "Did not understand state \"SHINY\" in color specification",
    );
    assert_rejects(
        &ops(r#"<rectangle color="black" x="0" y="0" width="1" height="1" filled="yes"/>"#),
        "Boolean values must be \"true\" or \"false\" not \"yes\"",
    );
    assert_rejects(
        &ops(r#"<arc color="black" x="0" y="0" width="1" height="1" extent_angle="90"/>"#),
        "No \"start_angle\" or \"from\" attribute on element <arc>",
    );
    assert_rejects(
        &ops(r#"<arc color="black" x="0" y="0" width="1" height="1" from="0"/>"#),
        "No \"extent_angle\" or \"to\" attribute on element <arc>",
    );
    // Marco's bounds are a million out, but they do bound.
    assert_rejects(
        &ops(
            r#"<arc color="black" x="0" y="0" width="1" height="1" start_angle="-2000000" extent_angle="90"/>"#,
        ),
        "Angle must be between 0.0 and 360.0, was -2000000",
    );
    assert_rejects(
        &ops(
            r#"<gtk_arrow state="shiny" shadow="in" arrow="up" x="0" y="0" width="1" height="1"/>"#,
        ),
        "Did not understand state \"shiny\" for <gtk_arrow> element",
    );
    assert_rejects(
        &ops(r#"<gtk_box state="normal" shadow="deep" x="0" y="0" width="1" height="1"/>"#),
        "Did not understand shadow \"deep\" for <gtk_box> element",
    );
    assert_rejects(
        &ops(
            r#"<gtk_arrow state="normal" shadow="in" arrow="sideways" x="0" y="0" width="1" height="1"/>"#,
        ),
        "Did not understand arrow \"sideways\" for <gtk_arrow> element",
    );
    assert_rejects(
        &ops(r#"<image filename="missing.png" x="0" y="0" width="1" height="1"/>"#),
        "Failed to load image \"missing.png\"",
    );
}

#[test]
fn frame_styles_are_checked() {
    assert_rejects(
        r#"<frame_style name="x"/>"#,
        "<frame_style> must specify either a geometry or a parent that has a geometry",
    );
    assert_rejects(
        r#"<frame_style name="x" parent="y"/>"#,
        "<frame_style> parent \"y\" has not been defined",
    );
    assert_rejects(
        r#"<frame_style name="x" geometry="y"/>"#,
        "<frame_style> geometry \"y\" has not been defined",
    );
    assert_rejects(
        r#"<frame_style name="x" geometry="g" alpha="0.5"/>"#,
        "You must specify a background for an alpha value to be meaningful",
    );
    let style = |body: &str| {
        format!(r#"<draw_ops name="x"/><frame_style name="x" geometry="g">{body}</frame_style>"#)
    };
    assert_rejects(
        &style(r#"<piece position="corner" draw_ops="x"/>"#),
        "Unknown position \"corner\" for frame piece",
    );
    assert_rejects(
        &style(r#"<piece position="title" draw_ops="x"/><piece position="title" draw_ops="x"/>"#),
        "Frame style already has a piece at position title",
    );
    assert_rejects(
        &style(r#"<piece position="title"/>"#),
        "No draw_ops provided for frame piece",
    );
    assert_rejects(
        &style(r#"<piece position="title" draw_ops="nope"/>"#),
        "No <draw_ops> with the name \"nope\" has been defined",
    );
    assert_rejects(
        &style(r#"<piece position="title" draw_ops="x"><draw_ops/></piece>"#),
        "Can't have a two draw_ops for a <piece> element",
    );
    assert_rejects(
        &style(r#"<button function="help" state="normal" draw_ops="x"/>"#),
        "Unknown function \"help\" for button",
    );
    assert_rejects(
        &style(r#"<button function="close" state="hover" draw_ops="x"/>"#),
        "Unknown state \"hover\" for button",
    );
    assert_rejects(
        &style(r#"<button function="close" state="normal"/>"#),
        "No draw_ops provided for button",
    );
    assert_rejects(
        &style(
            r#"<button function="close" state="normal" draw_ops="x"/><button function="close" state="normal" draw_ops="x"/>"#,
        ),
        "Frame style already has a button for function close state normal",
    );
    assert_rejects(
        &style(""),
        "<button function=\"close\" state=\"normal\" draw_ops=\"whatever\"/> must be specified for this frame style",
    );
}

#[test]
fn style_sets_are_checked() {
    assert_rejects(
        r#"<frame_style_set name="set"/>"#,
        "<frame_style_set> name \"set\" used a second time",
    );
    assert_rejects(
        r#"<frame_style_set name="x" parent="y"/>"#,
        "<frame_style_set> parent \"y\" has not been defined",
    );
    let set = |frame: &str| {
        format!(r#"<frame_style_set name="x" parent="set">{frame}</frame_style_set>"#)
    };
    assert_rejects(
        &set(r#"<frame focus="maybe" state="normal" resize="both" style="s"/>"#),
        "\"maybe\" is not a valid value for focus attribute",
    );
    assert_rejects(
        &set(r#"<frame focus="yes" state="minimized" resize="both" style="s"/>"#),
        "is not a valid value for state attribute",
    );
    assert_rejects(
        &set(r#"<frame focus="yes" state="normal" resize="all" style="s"/>"#),
        "is not a valid value for resize attribute",
    );
    assert_rejects(
        &set(r#"<frame focus="yes" state="normal" resize="both" style="t"/>"#),
        "A style called \"t\" has not been defined",
    );
    assert_rejects(
        &set(r#"<frame focus="yes" state="normal" style="s"/>"#),
        "No \"resize\" attribute on element <frame>",
    );
    assert_rejects(
        &set(r#"<frame focus="yes" state="maximized" resize="both" style="s"/>"#),
        "Should not have \"resize\" attribute on <frame> element for maximized states",
    );
    assert_rejects(
        &set(
            r#"<frame focus="yes" state="maximized" style="s"/><frame focus="yes" state="maximized" style="s"/>"#,
        ),
        "Style has already been specified for state maximized focus yes",
    );
    let text = replace(
        &document(3, ""),
        r#"<frame focus="no" state="shaded" style="s"/>"#,
        "",
    );
    assert!(reject_text(&text, 3).1.contains(
        "Missing <frame state=\"shaded\" resize=\"none\" focus=\"no\" style=\"whatever\"/>"
    ));
}
