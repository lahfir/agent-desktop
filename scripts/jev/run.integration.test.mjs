import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { execute } from "./desktop.mjs";

const directory = mkdtempSync(join(tmpdir(), "jev-loop-test-"));
const entry = join(dirname(fileURLToPath(import.meta.url)), "run.mjs");
const log = join(directory, "commands.jsonl");
const preload = join(directory, "fetch.mjs");

const fake = `
const { appendFileSync } = require('node:fs');
const { basename } = require('node:path');
const command = basename(process.argv[1]);
const args = process.argv.slice(2);
const mode = process.env.JEV_TEST_MODE;
appendFileSync(process.env.JEV_TEST_LOG, JSON.stringify({command,args})+'\\n');
const error = (code, retry='unsafe', delivery='delivery_uncertain') =>
  ({ok:false,error:{code,message:'試験の失敗',disposition:{retry,delivery}}});
let result = {ok:true,data:{disposition:{delivery:'delivered_verified',retry:'unsafe'}}};
const clicked = () => require('node:fs').readFileSync(process.env.JEV_TEST_LOG,'utf8').includes('"command":"click"');
if (command === 'snapshot' && mode === 'window_closed' && clicked()) result = error('WINDOW_NOT_FOUND','safe','not_delivered');
else if (command === 'snapshot' && mode === 'closed_top' &&
  require('node:fs').readFileSync(process.env.JEV_TEST_LOG,'utf8').split('\\n').filter(l=>l.includes('"command":"snapshot"')).length >= 3)
  result = error('WINDOW_NOT_FOUND','safe','not_delivered');
else if (command === 'snapshot') {
  const at = args.indexOf('--window-id');
  const actions = require('node:fs').readFileSync(process.env.JEV_TEST_LOG,'utf8').split('\\n').filter(Boolean)
    .map(JSON.parse).filter(item => ['click','set-value'].includes(item.command));
  const written = actions.filter(item => item.command === 'set-value').at(-1)?.args.at(-1) ?? '';
  const suppliedInputs = JSON.parse(process.env.JEV_TEST_INPUTS ?? '[]');
  const pinned = at === -1 ? 'w-default' : args[at + 1];
  const surfaced = args.includes('--surface');
  const rooted = args.includes('--root');
  let id = pinned;
  if (surfaced && mode === 'surface_other') id = 'w-other';
  if (rooted && mode === 'root_other') id = 'w-other';
  const window = surfaced && mode === 'surface_noid' ? {title:'試験'} : {title:'試験',id};
  const sheet = mode.startsWith('surface') && !surfaced ? [{role:'sheet',children:[]}] : [];
  result = {ok:true,data:{window,tree:{
    role:'window',children:[
      {ref_id:'@s1:e1',role:'button',name:mode==='sequence'&&actions.length>=1?suppliedInputs[1]:'試験ボタン',available_actions:['Click']},
      {ref_id:'@s1:e2',role:'textfield',name:'試験欄',value:process.env.JEV_ACT_TEST?'PRIVATE-ACT-SCREEN-VALUE-4583':mode==='sequence'?written:'',available_actions:['SetValue']},
      ...sheet
    ]}}};
  if (mode === 'skeleton' && !rooted) result.data.tree = {role:'window',children:[{ref_id:'@s1:e9',role:'group',children_count:3}]};
}
if (command === 'click' && mode === 'click_error') result=error('ACTION_FAILED');
if (command === 'set-value') result= mode==='unsafe_type'
  ? error('TIMEOUT') : mode==='old_binary'
  ? {ok:false,error:{code:'ACTION_NOT_SUPPORTED',message:'古い'}}
  : ['direct_type','sequence'].includes(mode)
  ? {ok:true,data:{disposition:{delivery:'delivered_verified'},post_state:{value:args.at(-1)},details:{value:args.at(-1)},steps:[{value:args.at(-1)}]}}
  : error('ACTION_NOT_SUPPORTED','safe','not_delivered');
if (command === 'focus' && mode==='focus_error') result=error('ACTION_FAILED','unsafe','not_delivered');
if (command === 'press' && mode==='press_error') result=error('ACTION_FAILED');
if (command === 'clipboard-get') result={ok:true,data:{text:'元の文字'}};
if (command === 'clipboard-set' && mode==='clipboard_error' && args[0]==='試験')
  result=error('COPY_FAILED','safe','not_delivered');
if (command === 'get') {
  const reads = require('node:fs').readFileSync(process.env.JEV_TEST_LOG,'utf8').split('\\n')
    .filter(l => l.includes('"command":"get"')).length;
  result = mode==='get_error' ? error('ACTION_FAILED','unsafe','not_delivered')
    : {ok:true,data:{value:mode==='paste_wrong'?'違う文字':mode==='paste_late'&&reads<3?'':process.env.JEV_TEST_INPUT}};
}
console.log(JSON.stringify(result));
`;
for (const command of ["snapshot", "click", "set-value", "focus", "clipboard-get", "clipboard-set", "press", "get"]) {
  writeFileSync(join(directory, command), fake);
}
writeFileSync(preload, `
import { appendFileSync } from 'node:fs';
let calls=0;
globalThis.fetch=async (_url,options)=>{
  calls++;
  const body=JSON.parse(options.body), mode=process.env.JEV_TEST_MODE;
  if (body.questions.destructive) {
    appendFileSync(process.env.JEV_TEST_LOG,JSON.stringify({command:'risk',args:[]})+'\\n');
    return {ok:true,status:200,json:async()=>({answers:{destructive:{noul:mode==='destructive'?0.99:0.1}}})};
  }
  if (['destructive','risk_safe'].includes(mode)) {
    appendFileSync(process.env.JEV_TEST_LOG,JSON.stringify({command:'goal',args:[body.state.goal]})+'\\n');
  }
  const operation=mode==='skeleton'?({1:'DRILL',2:'CLICK'}[calls]??'DONE'):['destructive','risk_safe'].includes(mode)?(calls===1?'CLICK':'DONE'):mode==='direct_type'?(calls===1?'TYPE_TEXT':'DONE'):mode==='sequence'?({1:'TYPE_TEXT',2:'CLICK',3:'TYPE_TEXT',4:'CLICK'}[calls]??'DONE'):mode==='wait'?(calls<=3?'WAIT':'DONE'):mode==='closed_top'?'WAIT':(mode==='done'||mode.startsWith('surface')||mode.endsWith('_other'))?'DONE':
    mode==='blocked'?'BLOCKED':mode==='click_error'||mode==='window_closed'?'CLICK':(mode==='paste_correct'||mode==='paste_late')&&calls>1?'DONE':'TYPE_TEXT';
  if (mode==='sequence') appendFileSync(process.env.JEV_TEST_LOG,JSON.stringify({command:'request',body})+'\\n');
  const answers={};
  for (const [id,question] of Object.entries(body.questions)) {
    const ids=Object.keys(question.criteria);
    const selected=id==='operation'?operation:ids[0];
    answers[id]={choice:selected,confidence:mode==='skeleton'&&id==='drill_target'?0.6:1,probabilities:Object.fromEntries(ids.map(x=>[x,Number(x===selected)]))};
  }
  return {ok:true,status:200,json:async()=>({answers})};
};
`);

const actEntry = join(dirname(fileURLToPath(import.meta.url)), "act.mjs");
const actPreload = join(directory, "act-fetch.mjs");
writeFileSync(actPreload, `
import { appendFileSync } from 'node:fs';
globalThis.fetch=async (_url,options)=>{
  const body=JSON.parse(options.body);
  appendFileSync(process.env.JEV_TEST_LOG,JSON.stringify({command:'request',body})+'\\n');
  const ref=Object.keys(body.questions.target.criteria)[0];
  const noul={type:'noul',noul:0.1};
  return {ok:true,status:200,json:async()=>({answers:{
    target:{type:'choice',choice:ref,confidence:0.95,probabilities:{[ref]:0.95}},
    command:{type:'choice',choice:'click',confidence:0.95},
    present:{type:'noul',noul:0.95},destructive:{type:'noul',noul:process.env.JEV_TEST_MODE==='destructive'?0.99:0.1},needs_text:noul}})};
};
`);

const act = (mode, extra = []) => {
  writeFileSync(log, "");
  const result = spawnSync(process.execPath, ["--import", pathToFileURL(actPreload).href, actEntry,
    "--app", "試験アプリ", "--bin", process.execPath, ...extra, "試験ボタンを押す"], {
    cwd: directory,
    encoding: "utf8",
    timeout: 10000,
    env: { ...process.env, TYPESAFE_API_KEY: "fixture", JEV_TEST_MODE: mode, JEV_TEST_LOG: log, JEV_ACT_TEST: "1" },
  });
  assert.ifError(result.error);
  const commands = readFileSync(log, "utf8").trim().split("\n").filter(Boolean).map(JSON.parse);
  return { ...result, commands, out: result.stdout.trim() ? JSON.parse(result.stdout) : null };
};

const run = (mode, extra = [], input = "試験") => {
  const inputs = Array.isArray(input) ? input : [input];
  writeFileSync(log, "");
  const result = spawnSync(process.execPath, ["--import", pathToFileURL(preload).href, entry,
    "--app", "試験アプリ", ...inputs.flatMap(value => ["--text", value]), ...extra, "試験を完了する"], {
    cwd: directory,
    encoding: "utf8",
    timeout: 10000,
    env: { ...process.env, JEV_VERIFY_MS: "600", TYPESAFE_API_KEY: "fixture", AGENT_DESKTOP_BIN: process.execPath,
      JEV_TEST_MODE: mode, JEV_TEST_LOG: log, JEV_TEST_INPUT: inputs[0], JEV_TEST_INPUTS: JSON.stringify(inputs) },
  });
  assert.ifError(result.error);
  assert.equal(result.stderr, "");
  const events = result.stdout.trim().split("\n").map(JSON.parse);
  const commands = readFileSync(log, "utf8").trim().split("\n").filter(Boolean).map(JSON.parse);
  return { ...result, events, commands, stop: events.at(-1) };
};

try {
  const confirmed = run("destructive", ["--confirm-destructive"]);
  assert.equal(confirmed.status, 1);
  assert.match(confirmed.stop.stop, /^confirm: hard to undo/);
  assert.equal(confirmed.commands.filter(c => c.command === "risk").length, 1);
  assert.equal(confirmed.commands.some(c => c.command === "click"), false);
  assert.deepEqual(confirmed.commands.find(c => c.command === "goal").args, ["試験を完了する"]);
  const defaultRisk = run("destructive");
  assert.equal(defaultRisk.status, 0);
  assert.equal(defaultRisk.commands.some(c => c.command === "risk"), false);
  assert.equal(defaultRisk.commands.filter(c => c.command === "click").length, 1);
  const safeRisk = run("risk_safe", ["--confirm-destructive"]);
  assert.equal(safeRisk.status, 0);
  assert.equal(safeRisk.commands.filter(c => c.command === "risk").length, 1);
  assert.equal(safeRisk.commands.filter(c => c.command === "click").length, 1);
  const actConfirmed = act("destructive", ["--execute", "--confirm-destructive"]);
  assert.equal(actConfirmed.status, 1);
  assert.equal(actConfirmed.out.decision, "confirm");
  assert.equal(actConfirmed.out.intent, "試験ボタンを押す");
  assert.equal(actConfirmed.commands.some(c => c.command === "click"), false);
  for (const [mode, extra] of [["destructive", []], ["risk_safe", ["--confirm-destructive"]]]) {
    const allowed = act(mode, ["--execute", ...extra]);
    assert.equal(allowed.status, 0);
    assert.equal(allowed.out.decision, "act");
    assert.equal(allowed.commands.filter(c => c.command === "click").length, 1);
  }
  const started = performance.now();
  await execute({ app: "試験アプリ" }, "WAIT", null, null, null);
  assert.ok(performance.now() - started >= 230, "WAIT actually waits");

  const waited = run("wait");
  assert.equal(waited.status, 0);
  assert.equal(waited.stop.stop, "done");
  assert.equal(waited.events.filter(event => event.turn).length, 3);

  const drilled = run("skeleton");
  assert.equal(drilled.status, 0, "a skeleton of bare regions is drilled into, not reported as empty");
  assert.equal(drilled.stop.stop, "done");
  assert.deepEqual(drilled.events.filter(event => event.turn).map(event => event.turn.operation), ["DRILL", "CLICK"]);
  assert.equal(drilled.events[0].turn.confidence, 0.6, "looking is not held to the bar for acting");
  assert.ok(drilled.commands.some(c => c.command === "snapshot" && c.args.includes("--root") && c.args.includes("@s1:e9")));
  assert.equal(drilled.commands.filter(c => c.command === "click").length, 1);

  const failed = run("click_error");
  assert.equal(failed.status, 1);
  assert.equal(failed.stop.stop, "action_failed");
  assert.equal(failed.stop.error.code, "ACTION_FAILED");
  assert.equal(failed.stop.error.disposition.retry, "unsafe");
  assert.equal(failed.commands.filter(command => command.command === "click").length, 1);

  const unsafe = run("unsafe_type");
  assert.equal(unsafe.status, 1);
  assert.equal(unsafe.stop.error.code, "TIMEOUT");
  assert.equal(unsafe.commands.some(command => command.command === "focus" || command.command === "press"), false);

  const copyFailed = run("clipboard_error");
  assert.equal(copyFailed.status, 1);
  assert.equal(copyFailed.stop.error.code, "COPY_FAILED");
  assert.equal(copyFailed.commands.some(command => command.command === "press"), false);

  const wrongText = run("paste_wrong");
  assert.equal(wrongText.status, 1);
  assert.equal(wrongText.stop.error.code, "TEXT_VERIFICATION_FAILED");
  assert.equal(wrongText.commands.filter(command => command.command === "press").length, 1);
  assert.ok(wrongText.commands.filter(command => command.command === "get").length > 1, "a mismatch is re-read until the deadline");

  const supplied = "PRIVATE-RUN-INPUT-7391";
  const late = run("paste_late", [], supplied);
  assert.equal(late.status, 0);
  assert.equal(late.events[0].turn.delivery, "delivered_verified");
  assert.ok(late.events.filter(event => event.turn).every(event => !("text" in event.turn)), "stdout turns never retain supplied input");
  assert.ok(!late.stdout.includes(supplied), "stdout does not emit the supplied value");
  assert.equal(late.commands.filter(command => command.command === "press").length, 1, "the paste is not repeated");
  assert.equal(late.commands.filter(command => command.command === "get").length, 3);

  const direct = run("direct_type", [], supplied);
  assert.equal(direct.status, 0);
  assert.ok(!direct.stdout.includes(supplied), "stdout redacts supplied input echoed by action evidence");

  const secondSupplied = "PRIVATE-RUN-INPUT-7392";
  const sequence = run("sequence", [], [supplied, secondSupplied]);
  assert.equal(sequence.status, 0);
  assert.deepEqual(sequence.events.filter(event => event.turn).map(event => event.turn.operation), ["TYPE_TEXT", "CLICK", "TYPE_TEXT", "CLICK"]);
  assert.ok(!sequence.stdout.includes(supplied) && !sequence.stdout.includes(secondSupplied), "later target names and action evidence redact all supplied inputs");
  assert.ok(!sequence.events.find(event => event.turn?.operation === "CLICK").turn.target.includes(secondSupplied), "a target name is redacted before its matching text is entered");
  const historyRequests = sequence.commands.filter(command => command.command === "request").slice(1);
  assert.ok(historyRequests.every(({body}) => {
    const history = JSON.stringify(body.state.recent_actions);
    return !history.includes(supplied) && !history.includes(secondSupplied);
  }), "later model history never carries supplied strings back");

  const privateScreenValue = "PRIVATE-ACT-SCREEN-VALUE-4583";
  const actNoValues = act("done", ["--no-values"]);
  const actDefault = act("done");
  assert.ok(!JSON.stringify(actNoValues.commands.find(command => command.command === "request").body).includes(privateScreenValue));
  assert.ok(JSON.stringify(actDefault.commands.find(command => command.command === "request").body).includes(privateScreenValue));

  const old = run("old_binary");
  assert.equal(old.status, 1);
  assert.equal(old.stop.error.code, "BINARY_TOO_OLD");
  assert.equal(old.commands.some(command => command.command === "focus"), false);

  const focusFailed = run("focus_error");
  assert.equal(focusFailed.status, 1);
  assert.equal(focusFailed.stop.stop, "action_failed");
  assert.equal(focusFailed.stop.error.code, "ACTION_FAILED");
  assert.equal(focusFailed.events.filter(event => event.turn).at(-1).turn.ok, false);
  assert.equal(focusFailed.commands.some(command => command.command === "press"), false);

  const pressFailed = run("press_error");
  assert.equal(pressFailed.status, 1);
  assert.equal(pressFailed.stop.stop, "action_failed");
  assert.equal(pressFailed.stop.error.code, "ACTION_FAILED");
  assert.equal(pressFailed.commands.some(command => command.command === "get"), false);

  const readFailed = run("get_error");
  assert.equal(readFailed.status, 1);
  assert.equal(readFailed.stop.stop, "action_failed");
  assert.equal(readFailed.stop.error.code, "TEXT_VERIFICATION_FAILED");
  assert.equal(readFailed.stop.error.details.read_error.code, "ACTION_FAILED");

  const windowed = run("done", ["--window-id", "w-42"]);
  assert.equal(windowed.status, 0);
  const snapshots = windowed.commands.filter(command => command.command === "snapshot");
  assert.ok(snapshots.length > 0);
  for (const { args } of snapshots) assert.deepEqual(args.slice(args.indexOf("--window-id"), args.indexOf("--window-id") + 2), ["--window-id", "w-42"]);
  assert.equal(windowed.stop.screen.window_id, "w-42");
  assert.ok(snapshots.every(({ args }) => args.includes("--window-id")));
  assert.equal(run("done").commands.some(command => command.args.includes("--window-id")), false);

  const windowPaste = run("paste_correct", ["--window-id", "w-42"]);
  assert.equal(windowPaste.status, 0);
  const press = windowPaste.commands.find(command => command.command === "press");
  assert.deepEqual(press.args.slice(press.args.indexOf("--window-id"), press.args.indexOf("--window-id") + 2), ["--window-id", "w-42"]);

  const closed = run("window_closed", ["--window-id", "w-42"]);
  assert.equal(closed.status, 1);
  assert.equal(closed.stop.stop, "window_closed");
  assert.equal(closed.events.filter(event => event.turn).length, 1);
  assert.equal(closed.events.find(event => event.turn).turn.ok, true);
  const afterClick = closed.commands.slice(closed.commands.findIndex(command => command.command === "click") + 1);
  assert.equal(afterClick.filter(command => command.command === "snapshot").length, 1);

  const surfaced = run("surface_ok", ["--window-id", "w-42"]);
  assert.equal(surfaced.status, 0);
  const surfaceReads = surfaced.commands.filter(command => command.args.includes("--surface"));
  assert.ok(surfaceReads.length > 0, "the surface path runs");
  for (const { args } of surfaceReads) assert.deepEqual(args.slice(args.indexOf("--window-id"), args.indexOf("--window-id") + 2), ["--window-id", "w-42"]);
  assert.equal(surfaced.stop.screen.surface, "sheet");

  for (const mode of ["surface_other", "surface_noid"]) {
    const outside = run(mode, ["--window-id", "w-42"]);
    assert.equal(outside.status, 1, mode);
    assert.equal(outside.stop.stop, "surface_outside_window", mode);
    assert.equal(outside.commands.some(command => ["click", "set-value", "press"].includes(command.command)), false, mode);
  }

  const rootOutside = run("root_other", ["--window-id", "w-42", "--root", "@s1:e1"]);
  assert.equal(rootOutside.status, 1);
  assert.equal(rootOutside.stop.stop, "root_outside_window");
  const rootInside = run("done", ["--window-id", "w-42", "--root", "@s1:e1"]);
  assert.equal(rootInside.status, 0);

  const closedTop = run("closed_top", ["--window-id", "w-42"]);
  assert.equal(closedTop.status, 1);
  assert.equal(closedTop.stop.stop, "window_closed");
  assert.equal(closedTop.stop.error, undefined);
  assert.equal(closedTop.stop.screen.window_id, "w-42");

  for (const bad of ["abc", "Infinity", "-1"]) {
    writeFileSync(log, "");
    const refused = spawnSync(process.execPath, ["--import", pathToFileURL(preload).href, entry,
      "--app", "試験アプリ", "--text", "試験", "試験を完了する"], { cwd: directory, encoding: "utf8", timeout: 10000,
      env: { ...process.env, JEV_VERIFY_MS: bad, TYPESAFE_API_KEY: "fixture", AGENT_DESKTOP_BIN: process.execPath,
        JEV_TEST_MODE: "paste_wrong", JEV_TEST_LOG: log } });
    assert.notEqual(refused.status, 0, `JEV_VERIFY_MS=${bad} is refused`);
    assert.match(refused.stderr, /JEV_VERIFY_MS must be a number/);
    assert.equal(readFileSync(log, "utf8"), "", `JEV_VERIFY_MS=${bad} runs no command`);
  }

  for (const tail of [["--window-id"], ["--window-id", "--cursor"]]) {
    const extra = tail;
    const usage = spawnSync(process.execPath, [entry, "--app", "試験アプリ", "試験", ...extra],
      { encoding: "utf8", env: { ...process.env, TYPESAFE_API_KEY: "fixture" } });
    assert.equal(usage.status, 2, extra.join(" "));
    assert.match(usage.stderr, /needs a value/);
  }

  const actWindowed = act("surface_ok", ["--window-id", "w-42"]);
  assert.equal(actWindowed.status, 0);
  assert.equal(actWindowed.out.window_id, "w-42");
  assert.equal(actWindowed.out.surface, "sheet");
  const actReads = actWindowed.commands.filter(command => command.command === "snapshot");
  assert.equal(actReads.length, 2);
  for (const { args } of actReads) assert.deepEqual(args.slice(args.indexOf("--window-id"), args.indexOf("--window-id") + 2), ["--window-id", "w-42"]);

  for (const mode of ["surface_other", "surface_noid"]) {
    const outside = act(mode, ["--window-id", "w-42"]);
    assert.equal(outside.status, 1, mode);
    assert.equal(outside.out.error, "surface_outside_window", mode);
  }
  const actRoot = act("root_other", ["--window-id", "w-42", "--root", "@s1:e1"]);
  assert.equal(actRoot.status, 1);
  assert.equal(actRoot.out.error, "root_outside_window");
  const actUsage = act("done", ["--window-id", "--execute"]);
  assert.equal(actUsage.status, 2);

  assert.equal(run("blocked").status, 1);
  assert.equal(run("done").status, 0);
  const pasted = run("paste_correct");
  assert.equal(pasted.status, 0);
  assert.equal(pasted.commands.filter(command => command.command === "press").length, 1);
  assert.equal(pasted.events[0].turn.delivery, "delivered_verified");
  console.log("ok");
} finally {
  rmSync(directory, { recursive: true, force: true });
}
