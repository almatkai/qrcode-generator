import copy,json,pathlib,sys
from jsonschema import Draft202012Validator,FormatChecker
root=pathlib.Path(__file__).resolve().parent.parent
schema=json.loads((root/'integration/baseline/host/schemas/extension-v3.schema.json').read_text())
v=Draft202012Validator(schema,format_checker=FormatChecker());m=json.loads((root/'extension.json').read_text());v.validate(m)
for key,value in [('schemaVersion',2),('protocolVersion',2),('execution','compiled-in'),('id','com.tezbar.qrcode-generator'),('capabilities',['invented.method']),('replaces',[]),('unknown','bad')]:
 bad=copy.deepcopy(m);bad[key]=value;assert list(v.iter_errors(bad)),key
for p in sys.argv[1:]:v.validate(json.loads(pathlib.Path(p).read_text()))
print('Actual committed draft2020-12 schema: positive source/extracted manifests, seven negative cases passed.')
