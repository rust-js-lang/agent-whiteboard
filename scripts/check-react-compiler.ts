// Verify automatic optimization against rust-js output, without application-specific compiler configuration.
import {transform} from 'oxc-transform-react';
const modules=[['crates/web/src/App.jsx','App'],['crates/web/src/tools.js','useTools'],['crates/web/src/ui.jsx','Icon']] as const;
for(const [file,name] of modules){
 const source=await Bun.file(new URL('../'+file,import.meta.url)).text();
 const result=await transform(file,source,{jsx:{runtime:'automatic',development:false},reactCompiler:true});
 if(result.fatal||result.errors.length)throw Error(file+': '+result.errors.map(error=>error.message).join('\n'));
 const body=result.code.slice(result.code.indexOf('function '+name+'('));
 if(!result.code.includes('react/compiler-runtime')||!/^function \w+\([^]*?\{\s*(?:const|let|var) \$ = \w+\(\d+\)/.test(body.slice(0,1600)))throw Error(name+' was not automatically memoized');
 console.log('React Compiler optimized '+name+'.');
}
