# Simple ES Queries - Quick Reference

ES URL: `http://localhost:9300`
Index: `0xa038c593115f6fcd673f6833e15462b475994879`

---

## 1. Get All Documents (First 10)

```bash
curl -X POST "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "size": 10
}
'
```

---

## 2. Get Specific Token by ID

```bash
curl -X GET "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_doc/409192?pretty"
```

---

## 3. Filter by Owner

```bash
curl -X POST "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "size": 10,
  "query": {
    "term": {
      "owner": "0x42641bf6e50d32fdf6c73975cf9aa36555dece22"
    }
  }
}
'
```

---

## 4. Filter by Rarity (Common)

```bash
curl -X POST "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "size": 10,
  "query": {
    "term": {
      "attributes.rarity": "common"
    }
  }
}
'
```

---

## 5. Filter by Level Range (level >= 5)

```bash
curl -X POST "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "size": 10,
  "query": {
    "range": {
      "attributes.level": {
        "gte": "5"
      }
    }
  }
}
'
```

---

## 6. Multiple Filters (Rarity + Level + Type)

```bash
curl -X POST "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "size": 10,
  "query": {
    "bool": {
      "must": [
        { "term": { "attributes.rarity": "common" } },
        { "term": { "attributes.type": "archer" } },
        { "range": { "attributes.level": { "gte": "1" } } }
      ]
    }
  }
}
'
```

---

## 7. Filter by Price Range (if order_status is active)

```bash
curl -X POST "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "size": 10,
  "query": {
    "bool": {
      "must": [
        { "term": { "order_status": "active" } },
        { "range": { "price": { "gte": 100, "lte": 10000 } } }
      ]
    }
  },
  "sort": [
    { "price": { "order": "asc" } }
  ]
}
'
```

---

## 8. Search by Name

```bash
curl -X POST "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "size": 10,
  "query": {
    "match": {
      "name": "Archer"
    }
  }
}
'
```

---

## 9. Count Total Documents

```bash
curl -X GET "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_count?pretty"
```

---

## 10. Get Index Stats

```bash
curl -X GET "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_stats?pretty"
```

---

## Quick Test

Run this to verify ES is working:

```bash
curl -X GET "http://localhost:9300/_cluster/health?pretty"
```

