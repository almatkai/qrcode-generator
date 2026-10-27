import pathlib,importlib.util,tempfile,zipfile,stat,io,json,hashlib
root=pathlib.Path(__file__).resolve().parent.parent
spec=importlib.util.spec_from_file_location('private_package',root/'tools/package.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
with tempfile.TemporaryDirectory(dir=root/'.work',prefix='archive-negative-') as temp:
 temp=pathlib.Path(temp)
 for i,name in enumerate(['../escape','/absolute','C:/path','a\\b','a//b','a/./b','CON','A.','name ', 'bad\0x']):
  try:m.safe(name)
  except ValueError:pass
  else:raise AssertionError(name)
 for i,entries in enumerate([[('a',b'x',stat.S_IFLNK|0o777)],[('A',b'x',stat.S_IFREG|0o600),('a',b'x',stat.S_IFREG|0o600)]]):
  archive=temp/f'{i}.zip'
  with zipfile.ZipFile(archive,'w') as z:
   for name,data,mode in entries:
    info=zipfile.ZipInfo(name);info.create_system=3;info.external_attr=mode<<16;z.writestr(info,data)
  try:m.validate(archive,temp/f'extracted-{i}')
  except (AssertionError,ValueError):pass
  else:raise AssertionError('invalid archive accepted')
 try:m.header(b'not a native executable','aarch64-apple-darwin')
 except AssertionError:pass
 else:raise AssertionError('non-native bytes accepted')
print('Private ZIP member/symlink/case/Windows-path and native-header negative checks passed.')
