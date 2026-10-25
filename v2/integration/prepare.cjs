// Build narrow, UNAPPLIED patches from captured committed object bytes. No live host reads.
const fs=require('fs'),path=require('path'),cp=require('child_process');
const base=path.join(__dirname,'baseline/host'), out=path.join(__dirname,'proposed/host');
function change(file,edit) {const old=fs.readFileSync(path.join(base,file),'utf8');const next=edit(old);if(next===old)throw Error('No edit '+file);const p=path.join(out,file);fs.mkdirSync(path.dirname(p),{recursive:true});fs.writeFileSync(p,next);return file;}
const files=[];
files.push(change('src/renderer/src/raycast-api/index.tsx',s=>s
.replace('<FormRuntime\n', '<FormRuntime\n              key={props.sessionId}\n')
.replace('void onInvokeAction(id, formValues)','void onInvokeAction(id, formValues ?? (actions.find((action) => action.id === id)?.kind === \'submit-form\' ? formRuntimeRef.current?.getValues() : undefined))')
.replace('onChangeField={(actionId, value) => onInvokeAction(actionId, { value })}','onChangeField={(actionId, value, values) => onInvokeAction(actionId, { ...values, value })}')
.replace("if (kind === 'form' && action.id === submitAction?.id) {\n                  onSubmitForm(formRuntimeRef.current?.getValues() ?? {})","if (kind === 'form' && action.kind === 'submit-form') {\n                  void onInvokeAction(action.id, formRuntimeRef.current?.getValues() ?? {})")));
files.push(change('src/renderer/src/raycast-api/detail-runtime.tsx',s=>s.replace('onClick={onRunPrimaryAction}','onClick={() => onRunPrimaryAction()}')));
files.push(change('src/renderer/src/raycast-api/form-runtime.tsx',s=>s
.replace('type FormWheelCoordinates =',`// Opt-in versioned draft reconciliation for asynchronous controlled forms.
export function reconcileVersionedFormValues(
  fields: FormField[], previous: Record<string, unknown>, previousFields: FormField[],
  serverRevision: number, localRevisions: Record<string, number>
): Record<string, unknown> {
  const next = reconcileFormValues(fields, previous, previousFields)
  for (const field of fields) {
    if ((localRevisions[field.id] ?? 0) > serverRevision && Object.prototype.hasOwnProperty.call(previous, field.id)) next[field.id] = previous[field.id]
  }
  return next
}

type FormWheelCoordinates =`)
.replace('onChangeField: (actionId: string, value: unknown)', 'onChangeField: (actionId: string, value: unknown, values: Record<string, unknown>)')
.replace('const nativeColorWheel = useMemo',`const valuesRef = useRef(values)
  const localRevisions = useRef<Record<string, number>>({})
  const draftRevision = useRef(Number(root.props?.__tezbarDraftRevision) || 0)
  const versioned = typeof root.props?.__tezbarDraftRevision === 'number'
  const getDraft = (): Record<string, unknown> => versioned
    ? { ...valuesRef.current, __tezbarDraftRevision: draftRevision.current } : valuesRef.current
  const nativeColorWheel = useMemo`)
.replace('setValues((previous) => ({\n      ...reconcileFormValues(fields, previousFields ? previous : undefined, previousFields),\n      ...wheelDraft,\n    }))',`const serverRevision = Number(root.props?.__tezbarDraftRevision) || 0
    draftRevision.current = Math.max(draftRevision.current, serverRevision)
    const next = {
      ...(versioned ? reconcileVersionedFormValues(fields, valuesRef.current, previousFields ?? [], serverRevision, localRevisions.current)
        : reconcileFormValues(fields, previousFields ? valuesRef.current : undefined, previousFields)),
      ...wheelDraft,
    }
    valuesRef.current = next
    setValues(next)`)
.replace('}, [fields, nativeColorWheel, title])','}, [fields, nativeColorWheel, title, root.props?.__tezbarDraftRevision, versioned])')
.replace('useImperativeHandle(ref, () => ({ getValues: () => values }), [values])','useImperativeHandle(ref, () => ({ getValues: getDraft }))')
.replace('setValues((previous) => ({ ...previous, [field.id]: value }))\n    if (field.actionId) void onChangeField(field.actionId, value)',`if (versioned) {
      draftRevision.current += 1
      localRevisions.current[field.id] = draftRevision.current
    }
    const next = { ...valuesRef.current, [field.id]: value }
    valuesRef.current = next
    setValues(next)
    if (field.actionId) void onChangeField(field.actionId, value, getDraft())`)
.replaceAll('onSubmitForm(values)','onSubmitForm(getDraft())')));
files.push(change('src-tauri/src/native_core/extensions/registry.rs',s=>s
.replace('path: PathBuf,','path: PathBuf,\n    resource_path: PathBuf,')
.replace('let mut manifest = if native {','let mut resource_path = path.clone();\n            let mut manifest = if native {')
.replace('Ok(package) => package.manifest,','Ok(package) => { resource_path = package.dir.clone(); package.manifest },')
.replace('                path,\n                manifest,','                path,\n                resource_path,\n                manifest,')
.replace('        path,\n        manifest:', '        resource_path: path.clone(),\n        path,\n        manifest:')
.replaceAll('extension.path.join(relative)','extension.resource_path.join(relative)')
.replaceAll('extension.path.join("assets").join(relative)','extension.resource_path.join("assets").join(relative)')));
files.push(change('src/renderer/ExtensionRuntimeView.tsx',s=>s
.replace('export default function ExtensionRuntimeView', `export function isStaleDraftResponse(latest: number, response: unknown): boolean {
  const revision = (response as { root?: { props?: { __tezbarDraftRevision?: unknown } } })?.root?.props?.__tezbarDraftRevision
  return typeof revision === 'number' && revision < latest
}

export default function ExtensionRuntimeView`)
.replace('currentSession.current = sessionOwner', `const latestDraft = useRef({ owner: sessionOwner, revision: 0 })
  if (latestDraft.current.owner !== sessionOwner) latestDraft.current = { owner: sessionOwner, revision: 0 }
  currentSession.current = sessionOwner`)
.replace('onInvokeAction={async (actionId, formValues) => {', `onInvokeAction={async (actionId, formValues) => {
              const revision = formValues?.__tezbarDraftRevision
              if (typeof revision === 'number' && Number.isSafeInteger(revision)) latestDraft.current.revision = Math.max(latestDraft.current.revision, revision)`)
.replaceAll('if (currentSession.current !== sessionOwner) return', 'if (currentSession.current !== sessionOwner || isStaleDraftResponse(latestDraft.current.revision, result)) return')
.replace('if (requestSeq !== searchRequestSeq.current || currentSession.current !== sessionOwner) return','if (requestSeq !== searchRequestSeq.current || currentSession.current !== sessionOwner || isStaleDraftResponse(latestDraft.current.revision, result)) return')
.replace("if (cancelled || (result.ok && result.mode === 'unchanged')) return", "if (cancelled || isStaleDraftResponse(latestDraft.current.revision, result) || (result.ok && result.mode === 'unchanged')) return")));
let patch='';for(const f of files){let p=cp.spawnSync('diff',['-u','--label','a/'+f,'--label','b/'+f,path.join(base,f),path.join(out,f)],{encoding:'utf8'});if(p.status!==1)throw Error('Diff failed '+f);patch+=p.stdout;}
fs.writeFileSync(path.join(__dirname,'host-prerequisites.patch'),patch);
