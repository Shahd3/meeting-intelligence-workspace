import test from 'node:test';
import assert from 'node:assert/strict';
import { sessionView, needsRecovery } from '../src/data/recordingFlow.ts';
const status = (state, id = 'session-1') => ({state,id,elapsed_ms:45000,meeting_id:7,error:'retained audio',warning:'gap',peak:0.2});
test('repeated new forms hide every field from saved and failed sessions', () => {
  for (const state of ['saved','failed']) for(let opening=0;opening<4;opening++) {
    assert.deepEqual(sessionView(status(state), null), {state:'idle',id:null,elapsed_ms:0,meeting_id:null,error:null,warning:null,peak:0});
  }
});
test('navigation keeps all active states and their elapsed time', () => {
  for(const state of ['starting','recording','paused','stopping']) {
    const current=status(state); assert.equal(sessionView(current,null),current);
  }
});
test('failed save retains current session identity, errors and duration', () => {
  const failed=status('failed'); assert.equal(sessionView(failed,'session-1'),failed);
});
test('consecutive recordings never inherit the previous session', () => {
  assert.equal(sessionView(status('saved','previous'),null).elapsed_ms,0);
  assert.equal(sessionView(status('recording','next'),null).id,'next');
  assert.equal(sessionView(status('failed','previous'),'next').id,null);
});
test('recovery notice distinguishes interrupted partials from finalized or orphaned audio', () => {
  const recording=(state,filename)=>({state,tracks:[{filename}]});
  assert.equal(needsRecovery(recording('failed','microphone.part')),true);
  assert.equal(needsRecovery(recording('failed','microphone.wav')),false);
  assert.equal(needsRecovery(recording('saved','microphone.wav')),false);
  assert.equal(needsRecovery(recording('failed','microphone.recovered.wav')),false);
});
import { unavailableSources, savedMeetingAvailable } from '../src/data/recordingFlow.ts';
test('requested unavailable system audio cannot be replaced by microphone', () => {
  const selection={title:'test',microphone:true,system:true,microphone_device:null,system_device:null};
  const devices={microphones:[{id:'mic',name:'Microphone'}],outputs:[]};
  assert.deepEqual(unavailableSources(selection,devices),{microphone:false,system:true});
  assert.deepEqual(unavailableSources({...selection,system:false,microphone_device:'disconnected'},devices),{microphone:true,system:false});
  assert.deepEqual(unavailableSources(selection,null),{microphone:true,system:true});
  assert.deepEqual(unavailableSources(selection,{...devices,outputs:[{id:'speakers',name:'Speakers'}]}),{microphone:false,system:false});
});
test('failed save or failed refresh never allows return; a matching retry does', () => {
  assert.equal(savedMeetingAvailable(status('failed'),[{id:7}]),false);
  assert.equal(savedMeetingAvailable(status('stopping'),[{id:7}]),false);
  assert.equal(savedMeetingAvailable(status('saved'),undefined),false);
  assert.equal(savedMeetingAvailable(status('saved'),[{id:8}]),false);
  assert.equal(savedMeetingAvailable({...status('saved'),meeting_id:null},[{id:7}]),false);
  assert.equal(savedMeetingAvailable(status('saved'),[{id:7}]),true);
});
