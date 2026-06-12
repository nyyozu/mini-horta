// src/util.rs
//
// Funções utilitárias compartilhadas entre módulos de rotas.

/// Normaliza o nome da planta para busca/comparação: minúsculas, sem acentos.
/// Isso evita criar duplicatas como "Manjericao" e "Manjericão".
pub fn normalize_plant_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'á'|'à'|'â'|'ã'|'ä' => 'a',
            'é'|'è'|'ê'|'ë'     => 'e',
            'í'|'ì'|'î'|'ï'     => 'i',
            'ó'|'ò'|'ô'|'õ'|'ö' => 'o',
            'ú'|'ù'|'û'|'ü'     => 'u',
            'ç'                  => 'c',
            'ñ'                  => 'n',
            'Á'|'À'|'Â'|'Ã'|'Ä' => 'A',
            'É'|'È'|'Ê'|'Ë'     => 'E',
            'Í'|'Ì'|'Î'|'Ï'     => 'I',
            'Ó'|'Ò'|'Ô'|'Õ'|'Ö' => 'O',
            'Ú'|'Ù'|'Û'|'Ü'     => 'U',
            'Ç'                  => 'C',
            other                => other,
        })
        .collect::<String>()
        .to_lowercase()
}
