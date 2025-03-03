use clap::Parser;
use clap::Subcommand;
use epub::doc::EpubDoc;
use image::ImageReader;
use quick_xml::events::Event;
use quick_xml::Reader;
use zip::write::SimpleFileOptions;
use zip::ZipArchive;
use std::ffi::OsStr;
use std::fmt;
use std::fmt::Display;
use std::fs;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::{self};
use std::fs::File;
use anyhow::{anyhow, Result};
use zip::write::ZipWriter;

#[derive(Clone,Subcommand,PartialEq)]
enum BookFileFormat {
    Cbz,
    Zip,
    Epub
}

impl Display for BookFileFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BookFileFormat::Cbz => write!(f, "cbz"),
            BookFileFormat::Zip => write!(f, "zip"),
            BookFileFormat::Epub => write!(f, "epub"),
        }
    }
}

#[derive(Clone,Subcommand,PartialEq)]
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


#[derive(Clone,Subcommand,PartialEq)]
enum ImageFormat {
    Webp,
    Png,
    Jpeg,
}

impl Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageFormat::Webp => write!(f, "webp"),
            ImageFormat::Png => write!(f, "png"),
            ImageFormat::Jpeg => write!(f, "jpeg"),
        }
    }
}

#[derive(Parser)]
struct Args {
    #[arg(help = "ファイルパス")]
    file_path: Vec<String>, //Parse "aaa/xxx.cbz" "bbb/yyy.cbz" -> Vec["aaa/xxx.cbz","bbb/yyy.cbz"]

    #[arg(short = 'p', long = "export_path", help = "ファイルの出力先")]
    export_path: Option<String>, //Same Dir

    #[arg(short = 'f', long = "imageformat", help = "画像の出力形式")]
    export_image_type: Option<String>, //Unchanged

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
    let export_image_format= match args.export_image_type {
        Some(f) => match f.as_str() {
            "jpeg" => Some(ImageFormat::Jpeg),
            "png" => Some(ImageFormat::Png),
            "webp" => Some(ImageFormat::Webp),
            _ => None
        },
        None => None,
    };
    let export_path: Option<&Path> = None;//args.export_path.map_or(None, |f: String| Some(Path::new(f.as_str())));

    
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
        match book.convert(&export_file_format, &export_image_format, export_path) {
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

        result_drow(idx, args.file_path.len(), color, state, &file_name, &error_msg);
        idx += 1;
    }

    println!("[Finished!! \x1b[32mSuccess\x1b[m:{} \x1b[31mError\x1b[m:{} ExitStatus:{}]", succount, errcount, state);
   
}

fn result_drow(idx: usize, len: usize, color: usize, state: &str, file_name: &str, msg:&str){
    println!("#{}/{} \x1b[3{}m{}\x1b[m {} {}", idx, len, color, state, file_name, msg);
}


trait BookTrait {
    fn read_archive(&self) -> Result<ZipArchive<File>,anyhow::Error>;
    fn write_archive<P: AsRef<Path>>(&self, export_path: P) -> Result<ZipWriter<File>,anyhow::Error>;
    fn change_extension<P: AsRef<Path>>(&self, path: P, new_ext: &str) -> Result<(),anyhow::Error>;
    fn file_extension<P: AsRef<Path>>(&self, path: P) -> Result<BookFileFormat, anyhow::Error>;
    fn validate_folder<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error>;
    fn validate_file<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error>;
    fn image_convert(&self, export_image_format: &ImageFormat, bytes: &Vec<u8>, export_bytes: &mut Vec<u8>) -> Result<(), anyhow::Error>;
    fn convert<P: AsRef<Path>>(&self, export_file_format: &ExportBookFileFormat, export_image_format: &Option<ImageFormat>, export_path: Option<P>) -> Result<(), anyhow::Error>;
}

trait EpubTrait {
    fn fetch_image_path(&self) -> Result<Vec<String>,anyhow::Error>;
    fn epub_to_archive_file(&self, image_path: Vec<String>, rzip: &mut ZipArchive<File>, wzip: &mut ZipWriter<File>, export_image_format: &Option<ImageFormat>) -> Result<(), anyhow::Error>;
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

    fn change_extension<P: AsRef<Path>>(&self, path: P, new_ext: &str) -> Result<(),anyhow::Error> {
        let path = path.as_ref();
        let mut path_buf = path.to_path_buf();
        path_buf.set_extension(new_ext);
        fs::rename(path, path_buf)?;
        Ok(())
    }

    fn file_extension<P: AsRef<Path>>(&self, path: P) -> Result<BookFileFormat, anyhow::Error> {
        let path = path.as_ref();
        if let Some(x) = path.extension() {
            if let Some(str) = x.to_str() {
                match str {
                    "cbz" => return  Ok(BookFileFormat::Cbz),
                    "epub" => return Ok(BookFileFormat::Epub),
                    "zip" => return Ok(BookFileFormat::Zip),
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

    fn image_convert(&self, export_image_format: &ImageFormat, bytes: &Vec<u8>, export_bytes: &mut Vec<u8>) -> Result<(), anyhow::Error> {
        let img = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?.decode()?;
        let eif = match export_image_format {
            ImageFormat::Webp => image::ImageFormat::WebP,
            ImageFormat::Png => image::ImageFormat::Png,
            ImageFormat::Jpeg => image::ImageFormat::Jpeg,
        };  
        
        img.write_to(&mut Cursor::new(export_bytes), eif,)?;

        Ok(())
    }

    fn convert<P: AsRef<Path>>(&self, export_file_format: &ExportBookFileFormat, export_image_format: &Option<ImageFormat>, export_path: Option<P>) -> Result<(), anyhow::Error> {
         
        //出力先ディレクトリの設定
        let mut ep = self.book_file_path.to_path_buf();
        ep.pop();

        if let Some(x) = export_path {
            ep = x.as_ref().to_path_buf();
        }
        
        self.validate_folder(&ep)?;
        self.validate_file(&self.book_file_path)?;

        //読み込みファイルのフォーマットを取得
        let extension = self.file_extension(&self.book_file_path)?;

        //コンバート
        let file_name = match self.book_file_path.file_stem() {
            Some(f) => match f.to_str() {
                Some(f) => f,
                None => return Err(anyhow!("")),
            },
            None => return Err(anyhow!("")),
        };

        let mut rzip = self.read_archive()?;

        let bind = format!("{}.{}",&file_name,export_file_format.to_string());
        let save_path = ep.join(bind);
        let mut wzip = self.write_archive(&save_path)?;

        if extension == BookFileFormat::Epub {
            //Epubの処理
            let image_path_list = self.fetch_image_path()?;
            self.epub_to_archive_file(image_path_list,&mut rzip,&mut wzip,export_image_format)?;
        } else if extension == BookFileFormat::Cbz || extension == BookFileFormat::Zip  {
            //ZIP,CBR
            let mut file_index: i32 = 0;
            let mut buf = Vec::new();
            let mut buf2 = Vec::new();

            for rzip_index in 0..rzip.len() {
                buf.clear();
                buf2.clear();

                let options = SimpleFileOptions::default();
                let mut zip_file = match rzip.by_index(rzip_index){
                    Ok(f) => f,
                    Err(_) => continue,
                };

                let zip_file_mangled_name = &zip_file.mangled_name();
                let zip_file_name = zip_file_mangled_name.file_stem()
                .map_or("", |f| f.to_str().unwrap_or(""));
                let zip_file_extension = zip_file_mangled_name.extension()
                .map_or("", |f| f.to_str().unwrap_or(""));


                //拡張子を確認して要否を判定
                let mut is_image_format_convert = false;
                if let Some(ref x) = &export_image_format {
                    is_image_format_convert = x.to_string() != zip_file_extension;
                };

                if is_image_format_convert{
                    let eif = match &export_image_format{
                        Some(f) => f,
                        None => continue,
                    };
                    match zip_file.read_to_end(&mut buf) {
                        Ok(_) => (),
                        Err(_) => continue,
                    };

                    match self.image_convert(&eif,&buf,&mut buf2) {
                        Ok(_) => (),
                        Err(_) => continue,
                    };

                    let file_name = format!("{}.{}",zip_file_name , eif.to_string());
                    wzip.start_file(file_name, options)?;
                    wzip.write_all(&buf2)?;

                } else {
                    //RAWコピー
                    let file_name = format!("{}.{}",zip_file_name , zip_file_extension);
                    match wzip.raw_copy_file_rename(zip_file,&file_name){
                        Ok(_) => (),
                        Err(_) => continue,
                    };
                }

                file_index += 1;
            }
        }

        wzip.finish()?;

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

    fn epub_to_archive_file(&self, image_path: Vec<String>, rzip: &mut ZipArchive<File>, wzip: &mut ZipWriter<File>, export_image_format: &Option<ImageFormat>) -> Result<(), anyhow::Error> {
        let mut file_index: usize = 0;
        let mut image_path = image_path.clone();
        let options = SimpleFileOptions::default();
        let mut buf = Vec::new();
        let mut buf2: Vec<u8> = Vec::new();
        let mut image_path_index = Vec::new();
        for i in 0..(image_path.len() - 1) {
            image_path_index.push(i);
        }

        for rzip_index in 0..rzip.len() {
            let mut zip_file = rzip.by_index(rzip_index)?;
            let zip_file_mangled_name = &zip_file.mangled_name();
            let zip_file_name = zip_file_mangled_name.file_name()
            .map_or("", |f| f.to_str().unwrap_or(""));

            //ここでepub関数に分離
            for idx in 0..(image_path.len() - 1) {
                buf.clear();
                buf2.clear();
                
                let epub_path = Path::new(&image_path[idx]);
                let epub_file_name = epub_path.file_name()
                .map_or("", |f| f.to_str().unwrap_or("")) ;
                let epub_file_ext = epub_path.extension()
                .map_or("", |f| f.to_str().unwrap_or("")) ;
                
                //ファイル名が一致した場合ZIPアーカイブに抽出する
                if zip_file_name == epub_file_name{

                    //画像変換の要否を判定
                    let mut is_image_format_convert = false;
                    if let Some(ref x) = &export_image_format {
                        is_image_format_convert = x.to_string() != epub_file_ext;
                    };

                    if is_image_format_convert {
                        //画像変換後にZIPに格納
                        let option_ext = match export_image_format {
                            Some(ref f) => f,
                            None => break,
                        };

                        let _ = zip_file.read_to_end(&mut buf)?;
    
                        match self.image_convert(&option_ext,&buf,&mut buf2){
                            Ok(_) => (),
                            Err(e) => {
                                println!("{e}");
                                break;
                            },
                        };
                        
                        let file_name = format!("{}.{}", image_path_index[idx] , option_ext.to_string());
                        match wzip.start_file(&file_name, options){
                            Ok(_) => (),
                            Err(e) => {
                                println!("{e}");
                                break;
                            },
                        };
                        
                        match wzip.write_all(&buf2){
                            Ok(_) => (),
                            Err(e) => {
                                println!("{e}");
                                break;
                            },
                        };

                        wzip.flush()?;
                        
                        println!("Write:{file_name}")
                    } else {
                        //RAWコピー
                        let file_name = format!("{}.{}",image_path_index[idx] , epub_file_ext.to_string());
                        match wzip.raw_copy_file_rename(zip_file,&file_name){
                            Ok(_) => (),
                            Err(_) => break,
                        };
                    }
                    
                    file_index += 1;
                    image_path.remove(idx);
                    image_path_index.remove(idx);
                    break
                }
            }
        }
        Ok(())
    }
}









