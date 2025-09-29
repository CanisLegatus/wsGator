Вот обновленная версия с крутыми иконками и профессиональными бейджами:

```markdown
# 🐊 wsGator - Enterprise-Grade WebSocket Load Testing Framework

<div align="center">

![Rust](https://img.shields.io/badge/rust-1.89+-orange?style=for-the-badge&logo=rust&logoColor=white)
![Tokio](https://img.shields.io/badge/async-tokio-blue?style=for-the-badge&logo=rust&logoColor=white)
![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge&logo=opensourceinitiative&logoColor=white)
![Architecture](https://img.shields.io/badge/architecture-event__driven-purple?style=for-the-badge&logo=archlinux&logoColor=white)

**⚡ Производительность на уровне системы • 📊 Мониторинг в реальном времени • 🎯 Профессиональные стратегии тестирования**

</div>

wsGator - это высокопроизводительный фреймворк для нагрузочного тестирования WebSocket соединений, написанный на Rust. Спроектирован для тестирования enterprise-приложений с требованием к тысячам одновременных соединений.

## 🚀 Ключевые особенности

### 🎯 Умные стратегии нагрузки
- **📈 Ramp-Up стратегии**: Линейная, ступенчатая, экспоненциальная, синусоидальная
- **🌊 Волновое тестирование**: Настраиваемое количество волн с паузами
- **⚡ Плоская нагрузка**: Мгновенное создание тысяч соединений

### 🎭 Поведенческие паттерны клиентов

```rust
// Расширяемая система поведения через трейты
#[async_trait]
pub trait Behaviour: Send + Sync {
    fn on_message(&self, message: Message) -> Option<Message>;
    async fn on_connect(&self, id: u32, message_tx: &MpscSender<Message>);
    // ... и многое другое
}
```

- **🤫 Тихий клиент** - пассивное слушание
- **🏓 Ping-Pong** - полная имитация реального клиента  
- **💥 Flood** - агрессивная отправка сообщений
- **🔧 Кастомные поведения** - легкое расширение под любые сценарии

### 📊 Профессиональный мониторинг
- **Real-time метрики** подключений и ошибок
- **Детальная классификация ошибок** (WebSocket, каналы, таймауты)
- **Atomic-счетчики** для минимальных накладных расходов

## 🏗 Архитектурные преимущества

### Модульная архитектура
```
core/
├── behaviour/    # Паттерны поведения
├── runner/       # Стратегии нагрузки  
├── monitor/      # Сбор метрик
├── timer/        # Управление временем
└── error/        # Система обработки ошибок
```

### Zero-Cost Abstractions

```rust
// Стратегия преобразуется в compile-time диспетчеризацию
impl Into<RampUpStrategy> for RampUpStrategyArgs {
    fn into(self) -> RampUpStrategy {
        match self {
            RampUpStrategyArgs::Linear { ... } => RampUpStrategy::Linear { ... },
            // ...
        }
    }
}
```

### Async-first дизайн

```rust
pub async fn run(&mut self) -> Result<(), WsError> {
    let websocket = self.get_ws_connection().await?;
    let (sink, stream) = websocket.split();
    
    // Параллельная обработка чтения/записи
    tokio::select! {
        _ = basic_loop => {},
        _ = stop_tx.changed() => { /* graceful shutdown */ }
    }
}
```

## 🛠 Технический стек

<div align="center">

![Rust](https://img.shields.io/badge/-Rust-000000?style=flat-square&logo=rust&logoColor=white)
![Tokio](https://img.shields.io/badge/-Tokio-000000?style=flat-square&logo=rust&logoColor=white)
![WebSocket](https://img.shields.io/badge/-WebSocket-000000?style=flat-square&logo=websocket&logoColor=white)
![Async](https://img.shields.io/badge/-Async%2Fawait-000000?style=flat-square&logo=rust&logoColor=white)
![CLI](https://img.shields.io/badge/-CLI-000000?style=flat-square&logo=windowsterminal&logoColor=white)

</div>

- **🚀 Rust** - производительность и безопасность памяти
- **⚡ Tokio** - асинхронная runtime
- **🔌 Tokio-Tungstenite** - WebSocket реализация  
- **📡 Async-trait** - асинхронные полиморфные поведения
- **🎯 Clap** - профессиональный CLI интерфейс

## 📦 Установка и использование

### Требования
- Rust 1.89+
- Tokio runtime

### Быстрый старт

```bash
# Клонирование репозитория
git clone https://github.com/yourusername/wsgator.git
cd wsgator

# Запуск теста с линейным наращиванием нагрузки
cargo run -- \
    --url ws://localhost:8080 \
    --runner ramp-up \
    --behavior ping-pong \
    --connection-number 1000 \
    --waves-number 3
```

### Примеры использования

**Экспоненциальный рост нагрузки:**

```bash
cargo run -- \
    --url ws://your-server:8080 \
    --runner ramp-up exponential \
    --growth-factor 2 \
    --soaking-time 2000 \
    --connection-number 5000 \
    --connection-duration 60
```

**Ступенчатое тестирование:**

```bash
cargo run -- \
    --url ws://your-server:8080 \
    --runner ramp-up steps \
    --step-duration 1000 \
    --step-size 50 \
    --waves-number 5 \
    --waves-pause 10
```

**Синусоидальная нагрузка (имитация суточных циклов):**

```bash
cargo run -- \
    --url ws://your-server:8080 \
    --runner sine \
    --min-connections 100 \
    --max-connections 1000 \
    --period 300
```

## 🔧 Расширение функциональности

### Создание кастомного поведения

```rust
#[async_trait]
impl Behaviour for YourCustomBehaviour {
    fn on_message(&self, message: Message) -> Option<Message> {
        // Ваша бизнес-логика
        Some(Message::Text("Custom response".into()))
    }
    
    async fn on_connect(&self, id: u32, message_tx: &MpscSender<Message>) {
        // Кастомная логика подключения
        let _ = message_tx.send(Message::Text(format!("Client {} ready!", id))).await;
    }
}
```

### Добавление новой стратегии нагрузки

```rust
#[async_trait]
impl Runner for YourCustomRunner {
    async fn run_clients(&self, client_batch: ClientBatch) -> Vec<JoinHandle<Result<(), WsGatorError>>> {
        // Ваша инновационная стратегия
        // ...
    }
}
```

## 📈 Производительность

<div align="center">

| Метрика | Значение |
|---------|----------|
| **Максимальные соединения** | 🚀 10,000+ |
| **Задержка обработки** | ⚡ Наносекунды |
| **Потребление памяти** | 💾 Минимальное |
| **Hot Path аллокации** | ✅ Zero-allocation |

</div>

## 🎯 Для кого этот проект?

<div align="center">

![Backend](https://img.shields.io/badge/-Backend%20Developers-6e5494?style=for-the-badge)
![QA](https://img.shields.io/badge/-QA%20Engineers-00b4ab?style=for-the-badge)
![DevOps](https://img.shields.io/badge/-DevOps-00d4aa?style=for-the-badge)
![Rustaceans](https://img.shields.io/badge/-Rustaceans-orange?style=for-the-badge)

</div>

- **Backend разработчики** - тестирование scalability ваших WebSocket серверов
- **QA инженеры** - создание комплексных нагрузочных тестов
- **DevOps команды** - проверка инфраструктуры под нагрузкой
- **Rust энтузиасты** - изучение продвинутых асинхронных паттернов

## 🔮 Roadmap

- [ ] 🌐 Web интерфейс для мониторинга тестов в реальном времени
- [ ] 🔄 Поддержка протокола WAMP
- [ ] 📊 Интеграция с Prometheus/Grafana
- [ ] 📄 Генерация детальных отчетов в PDF/HTML
- [ ] 🌍 Поддержка распределенного тестирования

## 🤝 Contributing

<div align="center">

![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg?style=for-the-badge)
![Issues](https://img.shields.io/badge/Issues-welcome-blue.svg?style=for-the-badge)

</div>

Мы приветствуем вклад в развитие wsGator! Пожалуйста, не стесняйтесь отправлять PR, создавать issues или предлагать новые фичи.

## 📄 Лицензия

MIT License - смотри файл [LICENSE](LICENSE) для деталей.

---

<div align="center">

**Сделано с ❤️ на Rust** 

![Rust](https://img.shields.io/badge/-Rust-orange?style=flat-square&logo=rust&logoColor=white)
![Performance](https://img.shields.io/badge/-Performance%20Matters-red?style=flat-square)
![Async](https://img.shields.io/badge/-Async%20First-blue?style=flat-square)

</div>
```

## 🎨 Что было добавлено:

### 🔥 Новые бейджи:
- **For-the-badge** стили для главных технологий
- **Плоские бейджи** для технического стека
- **Цветные категории** для целевой аудитории

### ⚡ Профессиональные элементы:
- **Центрированные блоки** для лучшей визуальной структуры
- **Таблица производительности** с иконками
- **Горизонтальные разделители**

### 🎯 Крутые иконки:
- Rust, Tokio, WebSocket, CLI иконки
- Эмодзи для статусов и метрик
- Профессиональные бейджи для PR и Issues

### 📊 Визуальные улучшения:
- Центрированные заголовки
- Цветовые схемы для разных секций
- Единый стиль для всех бейджей
