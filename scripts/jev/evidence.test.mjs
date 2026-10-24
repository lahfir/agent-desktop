import assert from "node:assert/strict";
import {deliveryEvidence} from "./desktop.mjs";
import {actionSpace,buildRequest} from "./policy.mjs";
const steps=[{label:"AlreadyInState",outcome:"skipped",verified:true}];
const evidence=deliveryEvidence({ok:true,data:{disposition:{delivery:"not_delivered"},steps,post_state:{value:"0"}}});
assert.equal(evidence.ok,true);
assert.equal(evidence.delivery,"not_delivered");
assert.deepEqual(evidence.steps,steps);
assert.equal(evidence.post_state.value,"0");
const failed=deliveryEvidence({ok:false,error:{code:"POSTCONDITION_FAILED",disposition:{delivery:"delivered_unverified"},details:{after_action:{steps:[{label:"AXPress",outcome:"failed",verified:false}],post_state:{value:"1"}}}}});
assert.equal(failed.ok,false);
assert.equal(failed.delivery,"delivered_unverified");
assert.equal(failed.error.code,"POSTCONDITION_FAILED");
assert.equal(failed.steps[0].verified,false);
assert.equal(failed.post_state.value,"1");
const space=actionSpace([{ref_id:"@ref",role:"checkbox",name:"Public",value:"0",available_actions:["Toggle"]}]);
const turn={operation:"UNCHECK",ok:true,...evidence};
const body=buildRequest("uncheck Public",{app:"Fixture"},space,[turn]);
assert.equal(body.state.recent_actions[0].operation,"UNCHECK");
assert.ok(!("steps" in body.state.recent_actions[0]));
assert.ok(!("post_state" in body.state.recent_actions[0]));
assert.ok(!("details" in body.state.recent_actions[0]));
const secret="synthetic-private-input-7391";
const privateTurn={step:1,operation:"TYPE_TEXT",target:"textfield Account",ref:"@ref",confidence:0.95,
  ok:true,delivery:"delivered_verified",route:"set-value",truncated:false,changed:true,
  text:secret,error:{message:secret},details:{value:secret},steps:[{value:secret}],post_state:{value:secret}};
for (const values of [false,true]) {
  const request=buildRequest("fill Account",{app:"Fixture"},actionSpace([]),[privateTurn],{values});
  assert.ok(!JSON.stringify(request).includes(secret));
  assert.deepEqual(request.state.recent_actions,[{step:1,operation:"TYPE_TEXT",target:"textfield Account",ref:"@ref",
    confidence:0.95,ok:true,delivery:"delivered_verified",route:"set-value",truncated:false,changed:true}]);
}
assert.equal(privateTurn.text,secret);
console.log("ok");
