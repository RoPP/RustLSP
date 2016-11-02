// Copyright 2016 Bruno Medeiros
//
// Licensed under the Apache License, Version 2.0 
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>. 
// This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]


extern crate rust_lsp;


use rust_lsp::ls_types::*;
use rust_lsp::lsp_server::*;
use rust_lsp::jsonrpc::service_util::ServiceError;
use rust_lsp::jsonrpc::EndpointHandle;

use std::io;

pub struct DummyLanguageServer {
	server_endpoint : EndpointHandle,
}

pub fn run_lsp_server<OUT, OUT_P>(input: &mut io::BufRead, out_stream_provider: OUT_P)
where 
	OUT: io::Write + 'static, 
	OUT_P : FnOnce() -> OUT + Send + 'static
{
	let endpoint = LSPServer::new_lsp_endpoint(out_stream_provider);
	
	let ls = DummyLanguageServer{ server_endpoint : endpoint.clone() };
	
	LSPServer::run_server_from_input(ls, input, endpoint);
}

/**
 * A no-op language server
 */ 
impl DummyLanguageServer {
	
	// FIXME: user general error
	pub fn error_not_available<DATA>(data : DATA) -> ServiceError<DATA> {
		let msg = "Functionality not implemented.".to_string();
		ServiceError::<DATA> { code : 1, message : msg, data : data }
	}
	
}

impl LanguageServer for DummyLanguageServer {
	
	fn initialize(&self, _: InitializeParams) -> LSResult<InitializeResult, InitializeError> {
		let capabilities = ServerCapabilities::default();
		Ok(InitializeResult { capabilities : capabilities })
	}
	fn shutdown(&self, _: ()) -> LSResult<(), ()> {
		Ok(())
	}
	fn exit(&self, _: ()) {
	}
	
	fn workspaceChangeConfiguration(&self, _: DidChangeConfigurationParams) {}
	fn didOpenTextDocument(&self, _: DidOpenTextDocumentParams) {}
	fn didChangeTextDocument(&self, _: DidChangeTextDocumentParams) {}
	fn didCloseTextDocument(&self, _: DidCloseTextDocumentParams) {}
	fn didSaveTextDocument(&self, _: DidSaveTextDocumentParams) {}
	fn didChangeWatchedFiles(&self, _: DidChangeWatchedFilesParams) {}
	
	fn completion(&self, _: TextDocumentPositionParams) -> LSResult<CompletionList, ()> {
		Err(Self::error_not_available(()))
	}
	fn resolveCompletionItem(&self, _: CompletionItem) -> LSResult<CompletionItem, ()> {
		Err(Self::error_not_available(()))
	}
	fn hover(&self, _: TextDocumentPositionParams) -> LSResult<Hover, ()> {
		Err(Self::error_not_available(()))
	}
	fn signatureHelp(&self, _: TextDocumentPositionParams) -> LSResult<SignatureHelp, ()> {
		Err(Self::error_not_available(()))
	}
	fn gotoDefinition(&self, _: TextDocumentPositionParams) -> LSResult<Vec<Location>, ()> {
		Err(Self::error_not_available(()))
	}
	fn references(&self, _: ReferenceParams) -> LSResult<Vec<Location>, ()> {
		Err(Self::error_not_available(()))
	}
	fn documentHighlight(&self, _: TextDocumentPositionParams) -> LSResult<DocumentHighlight, ()> {
		Err(Self::error_not_available(()))
	}
	fn documentSymbols(&self, _: DocumentSymbolParams) -> LSResult<Vec<SymbolInformation>, ()> {
		Err(Self::error_not_available(()))
	}
	fn workspaceSymbols(&self, _: WorkspaceSymbolParams) -> LSResult<Vec<SymbolInformation>, ()> {
		Err(Self::error_not_available(()))
	}
	fn codeAction(&self, _: CodeActionParams) -> LSResult<Vec<Command>, ()> {
		Err(Self::error_not_available(()))
	}
	fn codeLens(&self, _: CodeLensParams) -> LSResult<Vec<CodeLens>, ()> {
		Err(Self::error_not_available(()))
	}
	fn codeLensResolve(&self, _: CodeLens) -> LSResult<CodeLens, ()> {
		Err(Self::error_not_available(()))
	}
	fn formatting(&self, _: DocumentFormattingParams) -> LSResult<Vec<TextEdit>, ()> {
		Err(Self::error_not_available(()))
	}
	fn rangeFormatting(&self, _: DocumentRangeFormattingParams) -> LSResult<Vec<TextEdit>, ()> {
		Err(Self::error_not_available(()))
	}
	fn onTypeFormatting(&self, _: DocumentOnTypeFormattingParams) -> LSResult<Vec<TextEdit>, ()> {
		Err(Self::error_not_available(()))
	}
	fn rename(&self, _: RenameParams) -> LSResult<WorkspaceEdit, ()> {
		Err(Self::error_not_available(()))
	}
}
