use clap::Parser;
use clap::Subcommand;
use epub::doc::EpubDoc;
use quick_xml::{events::Event, Reader};
use zip::ZipArchive;
use std::ffi::OsStr;
use std::fmt::{self, Display};
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
    file_path: Vec<String>, //Parse "aaa/xxx.cbz" "bbb/yyy.cbz" -> Vec["aaa/xxx.cbz","bbb/yyy.cbz"]

    #[arg(short = 'p', long = "export_path", help = "ファイルの出力先")]
    export_path: Option<String>, //Same Dir

    #[arg(short = 'i', long = "fileformat", default_value_t = ("cbz".to_string()), help = "ファイルの出力形式")]
    export_file_type: String, //cbz

    #[arg(short = 'd',  default_value_t = false ,help = "変換前のファイルを削除")]
    delete_file: bool, //False

}

fn main() {

    let args = Args::parse();

    let mut idx = 1;
    let mut succount = 0;
    let mut errcount = 0;
    let state = 0;

    let export_file_format= match args.export_file_type.as_str() {
        "cbz" => ExportBookFileFormat::Cbz,
        "zip" => ExportBookFileFormat::Zip,
        _ => ExportBookFileFormat::Cbz
    };


    let export_path: Option<&Path> = match args.export_path{
        Some(ref t) => Some(Path::new(t)),
        None => None,
    };

    let mut res_log: Vec<String> = Vec::new();
    
    for path in &args.file_path{
        let mut error = false;
        let mut state = "Success";
        let mut color = 2;
        let mut error_msg = "".to_string();

        let path = Path::new(path);
        let file_name = path.file_name()
        .unwrap_or(OsStr::new("None"))
        .to_str().unwrap_or("None");
        
        let book = Book::new(path);
        match book.convert(&export_file_format, export_path) {
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

        res_log.push(result_format(idx, args.file_path.len(), color, state, &file_name, &error_msg));
        idx += 1;
    }

    for res_str in res_log {
        println!("{}", res_str);
    }

    println!("[Finished!! \x1b[32mSuccess\x1b[m:{} \x1b[31mError\x1b[m:{} ExitStatus:{}]", succount, errcount, state);
   
}

fn result_format(idx: usize, len: usize, color: usize, state: &str, file_name: &str, msg:&str) -> String {
    format!("#{}/{} \x1b[3{}m{}\x1b[m {} {}", idx, len, color, state, file_name, msg)
}


trait BookTrait {
    fn read_archive(&self) -> Result<ZipArchive<File>,anyhow::Error>;
    fn write_archive<P: AsRef<Path>>(&self, export_path: P) -> Result<ZipWriter<File>,anyhow::Error>;
    fn file_extension_check<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error>;
    fn validate_folder<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error>;
    fn validate_file<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error>;
    fn convert<P: AsRef<Path>>(&self, export_file_format: &ExportBookFileFormat, export_path: Option<P>) -> Result<(), anyhow::Error>;
}

trait EpubTrait {
    fn fetch_image_path(&self) -> Result<Vec<String>,anyhow::Error>;
    fn epub_to_archive_file(&self, image_path: Vec<String>, rzip: &mut ZipArchive<File>, wzip: &mut ZipWriter<File>) -> Result<(), anyhow::Error>;
}

struct Book{
    book_file_path: PathBuf,
}

impl Book {
    fn new<P: AsRef<Path>>(path: P) -> Self{
        let path = path.as_ref();
        Book {book_file_path: path.to_path_buf()}
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
            return Err(anyhow!("フォルダが存在しません"))
        }

        //このパスがフォルダなのか確認
        if !path.is_dir() {
            return Err(anyhow!("指定したパスはフォルダではありません"))
        }

        Ok(())
    }

    fn validate_file<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error> {
        
        let path = path.as_ref();
        //ファイルが存在するか
        if !path.exists() {
            return Err(anyhow!("ファイルが存在しません"))
        }

        //このパスがファイルなのか確認
        if !path.is_file() {
            return Err(anyhow!("指定したパスはファイルではありません"))
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
        self.validate_file(&self.book_file_path)?;

        //ファイルのフォーマット
        let _ = self.file_extension_check(&self.book_file_path)?;

        let file_name = match self.book_file_path.file_stem(){
            Some(t) => match t.to_str() {
                Some(s) => s,
                None => "",
            },
            None => ""
        };

        if file_name.is_empty() {
            return Err(anyhow!("ファイル名を取得できません"));
        }
        
        let mut rzip = self.read_archive()?; 

        let bind = format!("{}.{}",&file_name,export_file_format.to_string());
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
    fn fetch_image_path(&self) -> Result<Vec<String>,anyhow::Error> {
        let doc = EpubDoc::new(&self.book_file_path);

        let mut epub = match doc {
            Ok(f) => f,
            Err(e) => {
                return Err(anyhow!(e));
            }
        };

        let mut buf = Vec::new();
        let mut image_path_list = Vec::new();

        let target_name = b"image";
        let target_name2 = b"xlink:href";
        
        for spine in epub.spine.clone() {
            let res = epub.get_resource(&spine);

            match res {
                //xml
                Ok(bytes) => {
                        
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
                                                    image_path_list.push(str::from_utf8(&attr.value.into_owned())?.to_owned());
                                                }
                                            }
                                            Err(e) => println!("Error: {:?}", e)
                                        }
                                    }
                                }
                                
                            buf.clear();
                        }
                    }
                    Err(e) => {
                        return Err(anyhow!(e));
                    }
                }

            }

        if image_path_list.is_empty(){
            return Err(anyhow!("Epub内の画像パスが見つかりませんでした"));
        }
        
        return Ok(image_path_list);
    }

    fn epub_to_archive_file(&self, image_path: Vec<String>, rzip: &mut ZipArchive<File>, wzip: &mut ZipWriter<File>) -> Result<(), anyhow::Error> {
        let mut image_path = image_path.clone();
        let mut image_path_index = Vec::new();

        for i in 0..(image_path.len() - 1) {
            image_path_index.push(i);
        }

        for rzip_index in 0..rzip.len() {
            let zip_file = rzip.by_index(rzip_index)?;
            let zip_file_mangled_name = &zip_file.mangled_name();
            let zip_file_name = zip_file_mangled_name.file_name()
            .map_or("", |f| f.to_str().unwrap_or(""));

            for idx in 0..(image_path.len() - 1) {
                
                let epub_path = Path::new(&image_path[idx]);
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









