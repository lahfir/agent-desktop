use agent_desktop_core::{Action, DeliverySemantics, ErrorCode};

use super::live_bench::Bench;
use super::live_bench_env::qt_environment;
use crate::tree::live::live_launch::{Launched, nonce};

const APP: &str = r#"
import json, pathlib, sys
from PyQt6.QtWidgets import QApplication, QWidget, QVBoxLayout, QComboBox, QListWidget
from PyQt6.QtCore import QTimer
app = QApplication([])
window = QWidget()
window.setWindowTitle(sys.argv[1])
layout = QVBoxLayout(window)
combo = QComboBox()
combo.addItems(['Alpha', 'Bravo'])
layout.addWidget(combo)
rows = QListWidget()
rows.addItems(['Row one', 'Row two'])
layout.addWidget(rows)
state = pathlib.Path(sys.argv[2])
def record():
    temporary = state.with_suffix('.tmp')
    temporary.write_text(json.dumps({'combo': combo.currentText(),
                                     'rows': [row.text() for row in rows.selectedItems()]}))
    temporary.replace(state)
combo.currentTextChanged.connect(record)
rows.itemSelectionChanged.connect(record)
record()
window.show()
QTimer.singleShot(0, combo.showPopup)
app.exec()
"#;

#[test]
#[ignore = "drives Qt in scripts/private-session.sh --wm"]
fn combo_popup_highlight_is_not_a_committed_value_but_plain_rows_remain_selectable() {
    let mut launched = Launched::new();
    let tag = nonce("combo-selection");
    let scratch = std::env::temp_dir().join(&tag);
    std::fs::create_dir_all(&scratch).unwrap();
    launched.scratch.push(scratch.clone());
    let state = scratch.join("selection.json");
    let (python, environment) = qt_environment();
    let pid = launched.spawn_with_env(
        &python,
        &["-c", APP, &tag, state.to_str().unwrap()],
        &environment,
    );
    let adapter = crate::LinuxAdapter::new();
    let frame = Bench::wait_for_frame(&adapter, &tag, pid, 20).expect("Qt window");
    let bench = Bench::attach(launched, frame);
    let option = bench.named("Bravo");
    let error = bench
        .act(&option, Action::Select("Bravo".to_owned()))
        .expect_err("popup highlight does not commit the combo");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let read = || {
        serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(&state).unwrap())
            .unwrap()
    };
    assert_eq!(read()["combo"], "Alpha");
    for name in ["Row two", "Row one"] {
        let row = bench.named(name);
        let outcome = bench.act(&row, Action::Select(name.to_owned())).unwrap();
        assert_eq!(
            outcome.disposition(),
            DeliverySemantics::delivered_verified()
        );
        bench
            .wait_for(|| (read()["rows"] == serde_json::json!([name])).then_some(()))
            .expect("independent Qt row selection changed");
    }
}
