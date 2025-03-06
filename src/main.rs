use clap::Parser;
use clap::Subcommand;
use epub::doc::EpubDoc;
use quick_xml::{events::Event, Reader};
use zip::ZipArchive;
use std::ffi::OsStr;
use std::fmt::{self, Display};
use std::io::stdin;
use std::path::{Path, PathBuf};
use std::str::{self};
use std::fs::File;
use anyhow::{anyhow, Result};
use zip::write::ZipWriter;

#[derive(Clone,Subcommand,PartialEq,Debug)]
enum ExportBookFileFormat {
    Cbz,
    Zip,
}

impl Display for ExportBookFileFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportBookFileFormat::Cbz => write!(f, "cbz"),
            ExportBookFileFormat::Zip => write!(f, "zip"),
        }
    }
}

#[derive(Parser,Debug)]
struct Args {
    #[arg(help = "ファイルパス")]
    path: Vec<String>, //Parse "aaa/xxx.epub" "bbb/yyy.epub" -> Vec["aaa/xxx.epub","bbb/yyy.epub"]

    #[arg(short = 'p', long = "export_path", help = "ファイルの出力先")]
    export_path: Option<String>, //Same Dir

    #[arg(short = 'y', long = "yes", help = "確認のスキップ")]
    check_skip: bool, 

    #[arg(short = 'i', long = "fileformat", default_value_t = ("cbz".to_string()), help = "ファイルの出力形式")]
    export_file_type: String, //cbz

    #[arg(short = 'd',  default_value_t = false ,help = "変換前のファイルを削除")] //Todo 未実装
    delete_file: bool, //False

}

fn main() {

    let args = Args::parse();

    let mut idx1 = 1;
    let mut idx2 = 1;
    let mut succount = 0;
    let mut errcount = 0;
    let mut state = "0";
    let mut book = Vec::new();
    let mut res_log: Vec<String> = Vec::new();

    let export_file_format= match args.export_file_type.as_str() {
        "cbz" => ExportBookFileFormat::Cbz,
        "zip" => ExportBookFileFormat::Zip,
        _ => ExportBookFileFormat::Cbz
    };

    let export_path: Option<&Path> = match args.export_path{
        Some(ref t) => Some(Path::new(t)),
        None => None,
    };

    let _ = fetch_epub_files(args.path,&mut book);
    
    let book_len = book.len();
    if book_len == 0 {
        println!("ファイルが見つかりません 処理を中断します");
        return
    }

    if !args.check_skip {
        for b in &book {
            println!("#{}/{} {}.{}", idx1, book_len, b.file_stem, b.file_extension);
            idx1 += 1;
        }
        
        println!("以下のファイルを変換しますか？ [y/n]");
    
        let mut ans = String::new();
        loop {
            stdin().read_line(&mut ans).ok();
            let res = ans.trim_end();
    
            match res {
                "y" =>  break,
                "n" => {
                    println!("処理を中断します");
                    return;
                }
                _ => {}
            }
    
            println!("y,nで入力してください [y/n]");
            ans.clear();
        } 
    }

    //変換
    for b in &book{
        let mut error = false;
        let mut state = "Success";
        let mut color = 2;
        let mut error_msg = "".to_string();

        match b.convert(&export_file_format, export_path) {
            Ok(_) => {},
            Err(e) => 
            {
                error = true;
                error_msg = e.to_string();
            },
        };

        if error {
            errcount += 1;
            state = "Error";
            color = 1;
        } else {
            succount += 1;
        }

        res_log.push(result_format(idx2, book_len, color, state, &b.file_stem, &error_msg));
        idx2 += 1;
    }

    for res_str in res_log {
        println!("{}", res_str);
    }

    println!("[Finished!! \x1b[32mSuccess\x1b[m:{} \x1b[31mError\x1b[m:{} ExitStatus:{}]", succount, errcount, state);
   
}

fn fetch_epub_files<T: std::convert::AsRef<OsStr>>(path:Vec<T>, book:&mut Vec<Book>) -> Result<(),anyhow::Error> {

    for s in &path{
        let p = Path::new(s);

        if p.exists() {
            if p.is_file(){
                if let Some(x ) = p.extension() {
                    if x == "epub" {
                        if let Ok(b) = Book::new(p) {
                            book.push(b);
                        };
                    }
                }
                continue;
            } else if p.is_dir(){
                
                let pt = p.read_dir()
                .map(|read_dir| 
                    read_dir.filter_map(
                        |dir_entry|{
                        let entry = dir_entry.ok()?;
                        Some(entry.path())
                        }
                    ).collect()
                );

               if let Ok(res) = pt {
                fetch_epub_files(res, book).ok();
               }

            }
        }
    }

    Ok(())
}

fn result_format(idx: usize, len: usize, color: usize, state: &str, file_name: &str, msg:&str) -> String {
    format!("#{}/{} \x1b[3{}m{}\x1b[m {} {}", idx, len, color, state, file_name, msg)
}


trait BookTrait {
    fn read_archive(&self) -> Result<ZipArchive<File>,anyhow::Error>;
    fn write_archive<P: AsRef<Path>>(&self, export_path: P) -> Result<ZipWriter<File>,anyhow::Error>;
    fn file_extension_check<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error>;
    fn validate_folder<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error>;
    fn convert<P: AsRef<Path>>(&self, export_file_format: &ExportBookFileFormat, export_path: Option<P>) -> Result<(), anyhow::Error>;
}

trait EpubTrait {
    fn fetch_image_path(&self) -> Result<Vec<PathBuf>,anyhow::Error>;
    fn xml_get_image_path(&self, bytes:Vec<u8>, image_path_list: &mut Vec<PathBuf>) -> Result<(),anyhow::Error>;
    fn epub_to_archive_file(&self, image_path: Vec<PathBuf>, rzip: &mut ZipArchive<File>, wzip: &mut ZipWriter<File>) -> Result<(), anyhow::Error>;
}

struct Book{
    book_file_path: PathBuf,
    file_stem: String,
    file_extension: String
}

impl Book{
    fn new<P: AsRef<Path>>(path: P) -> Result<Self, anyhow::Error>{
        let path = path.as_ref();

        //ファイルが存在するか
        if !path.exists() {
            return Err(anyhow!("ファイルが存在しません"))
        }

        //このパスがファイルなのか確認
        if !path.is_file() {
            return Err(anyhow!("指定したパスはファイルではありません"))
        }

        let file_extension = path.extension().map_or(
            Err(anyhow!("ファイルの拡張子が取得できません")), |f| {
                f.to_str().map_or(
                    Err(anyhow!("str型への変換に失敗しました")),|x| Ok(x.to_string()))})?;

        let file_stem = path.file_stem().map_or(
            Err(anyhow!("ファイル名が取得できません")), |f| {
                f.to_str().map_or(
                    Err(anyhow!("str型への変換に失敗しました")),|x| Ok(x.to_string()))})?;

        //ここでファイル一覧を取る？

        Ok(Book {book_file_path: path.to_path_buf(), file_stem, file_extension})
    }
}


impl BookTrait for Book {

    fn read_archive(&self) -> Result<ZipArchive<File>,anyhow::Error> {
        //アーカイブ準備
        let file = File::open(&self.book_file_path)?;
        let rzip = zip::ZipArchive::new(file)?;
        Ok(rzip)
    }

    fn write_archive<P: AsRef<Path>>(&self, export_path: P) -> Result<ZipWriter<File>,anyhow::Error> {
        let wzip = ZipWriter::new(File::create(export_path)?);
        Ok(wzip)
    }

    fn file_extension_check<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error> {
        let path = path.as_ref();

        if let Some(x) = path.extension() {
            if let Some(str) = x.to_str() {
                match str {
                    "epub" => return Ok(()),
                    _ => return Err(anyhow!("非対応の拡張子です")),
                }
            }
        }

        return Err(anyhow!("拡張子が特定できませんでした"))
    }

    fn validate_folder<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error> {
        let path = path.as_ref();
        //フォルダが存在するか
        if !path.exists() {
            return Err(anyhow!("出力先のフォルダが見つかりません"))
        }

        //このパスがフォルダなのか確認
        if !path.is_dir() {
            return Err(anyhow!("指定したパスはフォルダではありません"))
        }

        Ok(())
    }

    fn convert<P: AsRef<Path>>(&self, export_file_format: &ExportBookFileFormat, export_path: Option<P>) -> Result<(), anyhow::Error> {
         
        //出力先ディレクトリの設定
        let mut ep = self.book_file_path.to_path_buf();
        ep.pop();

        if let Some(x) = export_path {
            ep = x.as_ref().to_path_buf();
        }
        
        self.validate_folder(&ep)?;

        //ファイルのフォーマット
        let _ = self.file_extension_check(&self.book_file_path)?;
        
        let mut rzip = self.read_archive()?; 

        let bind = format!("{}.{}",&self.file_stem,export_file_format.to_string());
        let save_path = ep.join(bind);
        let mut wzip = self.write_archive(&save_path)?;

        //Epubの処理
        let image_path_list = self.fetch_image_path()?;
        self.epub_to_archive_file(image_path_list,&mut rzip,&mut wzip)?;

        wzip.finish()?;

        //

        Ok(())
    }

} 

impl EpubTrait for Book {
    fn xml_get_image_path(&self, bytes:Vec<u8> , image_path_list: &mut Vec<PathBuf>) -> Result<(),anyhow::Error> {

        let mut buf = Vec::new();
        let target_name = b"image";
        let target_name2 = b"xlink:href";
    
        //xmlパースして画像パスを取得する
        let xml = str::from_utf8(&bytes)?;
        let mut reader = Reader::from_str(xml);
        reader.config_mut().expand_empty_elements = true;
    
            loop {
                let x = match reader.read_event_into(&mut buf) {
                    Ok(Event::Eof) => break,
                    Ok(Event::Start(e)) => e,
                    _ => continue,
                    };
    
                    //イメージのパスを探す
                    if x.name().local_name().as_ref() == target_name {
                        for attr in x.attributes() {
                            match attr {
                                Ok(attr) => {
                                    if &attr.key.as_ref() == target_name2 {
                                        println!("FindImage: {:?}",str::from_utf8(&attr.value)?);
                                        image_path_list.push(Path::new(str::from_utf8(&attr.value.into_owned())?).to_path_buf());
                                    }
                                }
                                Err(e) => println!("Error: {:?}", e)
                            }
                        }
                    }
                    
                buf.clear();
            }
    
            Ok(())
    }

    fn fetch_image_path(&self) -> Result<Vec<PathBuf>,anyhow::Error> {
        let doc = EpubDoc::new(&self.book_file_path);
        let mut image_path_list = Vec::new();

        let mut epub = match doc {
            Ok(f) => f,
            Err(e) => {
                return Err(anyhow!(e));
            }
        };

        loop {
            if let Ok(mime) =  epub.get_current_mime() {
                
                match mime.as_str() {
                    
                    "image/png" => epub.get_current_path().map_or((),|x| image_path_list.push(x)),

                    "image/jpeg" => epub.get_current_path().map_or((),|x| image_path_list.push(x)),
                    
                    "application/xhtml+xml" => {
                        if let Ok(bytes) = epub.get_current(){
                            self.xml_get_image_path(bytes, &mut image_path_list).map_or((), |x| ()) 
                        };
                    },
                    _ => {}
                };
            };

            
            if epub.go_next().is_err() {
                break;
            }
        } 
        
        if image_path_list.is_empty(){
            return Err(anyhow!("Epub内の画像パスが見つかりませんでした"));
        }
        
        Ok(image_path_list)
    }

    fn epub_to_archive_file(&self, image_path: Vec<PathBuf>, rzip: &mut ZipArchive<File>, wzip: &mut ZipWriter<File>) -> Result<(), anyhow::Error> {
        let mut image_path = image_path.clone();
        let mut image_path_index = Vec::new();
        println!("{}", image_path.len());
        for i in 0..(image_path.len()) {
            image_path_index.push(i);
        }

        for rzip_index in 0..rzip.len() {
            let zip_file = rzip.by_index(rzip_index)?;
            let zip_file_mangled_name = &zip_file.mangled_name();
            let zip_file_name = zip_file_mangled_name.file_name()
            .map_or("", |f| f.to_str().unwrap_or(""));
           

            for idx in 0..(image_path.len()) {
                
                let epub_path = &image_path[idx];
                match epub_path.file_name(){
                    Some(_) => (),
                    None => println!("fuck"),
                };
                let epub_file_name = epub_path.file_name()
                .map_or("", |f| f.to_str().unwrap_or("")) ;
                let epub_file_ext = epub_path.extension()
                .map_or("", |f| f.to_str().unwrap_or("")) ;
                

                //ファイル名が一致した場合ZIPアーカイブに抽出する
                if zip_file_name == epub_file_name{
                    //RAWコピー
                    let file_name = format!("{}.{}",image_path_index[idx] , epub_file_ext.to_string());
                    match wzip.raw_copy_file_rename(zip_file,&file_name){
                        Ok(_) => (),
                        Err(_) => break,
                    };

                    image_path.remove(idx);
                    image_path_index.remove(idx);
                    break
                }
            }
        }
        Ok(())
    }
}









