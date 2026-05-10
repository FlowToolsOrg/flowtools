# Plugin Features

Plugin Features 是 plugins 的一个模块，用来挂载 plugin 自定义的功能函数。

## Why do this

在此项目的插件开发最佳实践中，插件的功能需要 `可cli化`，但是，要求每位开发者都去实现一个 `cli` 是非常困难的，因此，我们通过 `plugin-features` 模块来挂载插件的功能函数，开发者只需要实现功能函数即可，无需关心 `cli` 的实现。

## Type

```ts
type FeatureTag = string

type PluginFeatures = {
  [key: string]: {
    name: string
    description?: string
    version?: string
    tags: FeatureTag[]
  }
}
```
